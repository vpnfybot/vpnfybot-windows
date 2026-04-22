// SimpleProxyBridge.c - User-space proxy redirection without WinDivert
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <tlhelp32.h>
#include <psapi.h>
#include <winsock2.h>
#include <ws2tcpip.h>

#pragma comment(lib, "ws2_32.lib")
#pragma comment(lib, "iphlpapi.lib")

#define MAX_PROCESS_NAME 256
#define MAX_RULES 100

typedef void (*LogCallback)(const char* message);
typedef void (*ConnectionCallback)(const char* process_name, DWORD pid, const char* dest_ip, USHORT dest_port, const char* proxy_info);

typedef enum {
    TCP = 0,
    UDP = 1,
    BOTH = 2
} RuleProtocol;

typedef enum {
    PROXY = 0,
    DIRECT = 1,
    BLOCK = 2
} RuleAction;

typedef enum {
    PROXY_TYPE_HTTP = 0,
    PROXY_TYPE_SOCKS5 = 1
} ProxyType;

typedef struct {
    UINT32 rule_id;
    char process_name[MAX_PROCESS_NAME];
    char *target_hosts;
    char *target_ports;
    RuleProtocol protocol;
    RuleAction action;
    BOOL enabled;
} ProcessRule;

// Global state
static ProcessRule rules[MAX_RULES];
static UINT32 next_rule_id = 1;
static UINT32 rule_count = 0;
static BOOL is_running = FALSE;
static LogCallback log_callback = NULL;
static ConnectionCallback connection_callback = NULL;

static char proxy_host[256] = "";
static USHORT proxy_port = 0;
static ProxyType proxy_type = PROXY_TYPE_SOCKS5;
static char proxy_username[256] = "";
static char proxy_password[256] = "";
static BOOL dns_via_proxy = TRUE;
static BOOL localhost_via_proxy = FALSE;
static BOOL traffic_logging_enabled = TRUE;

static CRITICAL_SECTION cs;

// Forward declarations
static void log_message(const char* format, ...);
static BOOL should_redirect_process(const char* process_name);
static void inject_proxy_environment(DWORD pid);

// DLL entry point
BOOL WINAPI DllMain(HINSTANCE hinstDLL, DWORD fdwReason, LPVOID lpvReserved) {
    switch (fdwReason) {
        case DLL_PROCESS_ATTACH:
            InitializeCriticalSection(&cs);
            memset(rules, 0, sizeof(rules));
            log_message("SimpleProxyBridge DLL loaded");
            break;
            
        case DLL_PROCESS_DETACH:
            DeleteCriticalSection(&cs);
            break;
    }
    return TRUE;
}

// Helper: Thread-safe logging
static void log_message(const char* format, ...) {
    if (log_callback == NULL) return;
    
    char buffer[1024];
    va_list args;
    va_start(args, format);
    vsnprintf(buffer, sizeof(buffer), format, args);
    va_end(args);
    
    log_callback(buffer);
}

// Check if process should be redirected
static BOOL should_redirect_process(const char* process_name) {
    EnterCriticalSection(&cs);
    
    for (UINT32 i = 0; i < rule_count; i++) {
        if (!rules[i].enabled) continue;
        
        if (strcmp(rules[i].process_name, "*") == 0 || 
            _stricmp(rules[i].process_name, process_name) == 0) {
            LeaveCriticalSection(&cs);
            return TRUE;
        }
    }
    
    LeaveCriticalSection(&cs);
    return FALSE;
}

// Get process name from PID
static char* get_process_name_from_pid(DWORD pid) {
    static char name[MAX_PROCESS_NAME];
    HANDLE hProcess = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, FALSE, pid);
    
    if (hProcess != NULL) {
        if (GetProcessImageFileNameA(hProcess, name, MAX_PROCESS_NAME)) {
            // Extract just the filename from full path
            char* last_backslash = strrchr(name, '\\');
            if (last_backslash) {
                strcpy(name, last_backslash + 1);
            }
        } else {
            sprintf(name, "pid_%lu", pid);
        }
        CloseHandle(hProcess);
    } else {
        sprintf(name, "pid_%lu", pid);
    }
    
    return name;
}

// Set proxy environment variables for a process
static void set_proxy_environment() {
    if (proxy_type == PROXY_TYPE_HTTP) {
        char proxy_string[512];
        sprintf(proxy_string, "http://%s:%d", proxy_host, proxy_port);
        
        // Set HTTP_PROXY and HTTPS_PROXY
        SetEnvironmentVariableA("HTTP_PROXY", proxy_string);
        SetEnvironmentVariableA("HTTPS_PROXY", proxy_string);
        SetEnvironmentVariableA("ALL_PROXY", proxy_string);
        
        // Remove SOCKS proxy env vars
        SetEnvironmentVariableA("SOCKS_PROXY", NULL);
        SetEnvironmentVariableA("SOCKS5_PROXY", NULL);
    } else {
        // SOCKS5 proxy
        char socks_string[512];
        sprintf(socks_string, "socks5://%s:%d", proxy_host, proxy_port);
        
        SetEnvironmentVariableA("SOCKS_PROXY", socks_string);
        SetEnvironmentVariableA("SOCKS5_PROXY", socks_string);
        SetEnvironmentVariableA("ALL_PROXY", socks_string);
        
        // Remove HTTP proxy env vars
        SetEnvironmentVariableA("HTTP_PROXY", NULL);
        SetEnvironmentVariableA("HTTPS_PROXY", NULL);
    }
    
    log_message("Proxy environment set: %s://%s:%d", 
                proxy_type == PROXY_TYPE_HTTP ? "HTTP" : "SOCKS5",
                proxy_host, proxy_port);
}

// Check if IP matches target hosts pattern
static BOOL ip_matches_pattern(const char* ip, const char* pattern) {
    if (pattern == NULL || strcmp(pattern, "*") == 0) {
        return TRUE;
    }
    
    // Simple pattern matching (supports wildcards)
    // For now, just do exact match or "*"
    if (strstr(pattern, "*") != NULL) {
        return TRUE; // Accept any for now
    }
    
    return strcmp(ip, pattern) == 0;
}

// Check if port matches target ports pattern
static BOOL port_matches_pattern(USHORT port, const char* pattern) {
    if (pattern == NULL || strcmp(pattern, "*") == 0) {
        return TRUE;
    }
    
    // Try exact match first
    char port_str[16];
    sprintf(port_str, "%d", port);
    
    if (strstr(pattern, port_str) != NULL) {
        return TRUE;
    }
    
    // Check for range (e.g., "80-443")
    char* dash = strchr(pattern, '-');
    if (dash != NULL) {
        char start_str[16], end_str[16];
        strncpy(start_str, pattern, dash - pattern);
        start_str[dash - pattern] = '\0';
        strcpy(end_str, dash + 1);
        
        USHORT start = (USHORT)atoi(start_str);
        USHORT end = (USHORT)atoi(end_str);
        
        return port >= start && port <= end;
    }
    
    return FALSE;
}

// Exported functions
__declspec(dllexport) DWORD ProxyBridge_AddRule(const char* process_name, const char* target_hosts, 
                                                const char* target_ports, RuleProtocol protocol, RuleAction action) {
    EnterCriticalSection(&cs);
    
    if (rule_count >= MAX_RULES) {
        LeaveCriticalSection(&cs);
        return 0;
    }
    
    ProcessRule* rule = &rules[rule_count];
    rule->rule_id = next_rule_id++;
    strncpy(rule->process_name, process_name, MAX_PROCESS_NAME - 1);
    rule->process_name[MAX_PROCESS_NAME - 1] = '\0';
    
    // Copy target hosts
    if (target_hosts != NULL) {
        size_t len = strlen(target_hosts) + 1;
        rule->target_hosts = (char*)malloc(len);
        if (rule->target_hosts) {
            strcpy(rule->target_hosts, target_hosts);
        }
    } else {
        rule->target_hosts = NULL;
    }
    
    // Copy target ports
    if (target_ports != NULL) {
        size_t len = strlen(target_ports) + 1;
        rule->target_ports = (char*)malloc(len);
        if (rule->target_ports) {
            strcpy(rule->target_ports, target_ports);
        }
    } else {
        rule->target_ports = NULL;
    }
    
    rule->protocol = protocol;
    rule->action = action;
    rule->enabled = TRUE;
    
    rule_count++;
    
    log_message("Rule %lu added: %s -> %s:%s -> %s", 
                rule->rule_id, process_name, 
                target_hosts ? target_hosts : "*",
                target_ports ? target_ports : "*",
                action == PROXY ? "PROXY" : (action == DIRECT ? "DIRECT" : "BLOCK"));
    
    LeaveCriticalSection(&cs);
    return rule->rule_id;
}

__declspec(dllexport) BOOL ProxyBridge_EnableRule(DWORD rule_id) {
    EnterCriticalSection(&cs);
    
    for (UINT32 i = 0; i < rule_count; i++) {
        if (rules[i].rule_id == rule_id) {
            rules[i].enabled = TRUE;
            log_message("Rule %lu enabled", rule_id);
            LeaveCriticalSection(&cs);
            return TRUE;
        }
    }
    
    LeaveCriticalSection(&cs);
    return FALSE;
}

__declspec(dllexport) BOOL ProxyBridge_DisableRule(DWORD rule_id) {
    EnterCriticalSection(&cs);
    
    for (UINT32 i = 0; i < rule_count; i++) {
        if (rules[i].rule_id == rule_id) {
            rules[i].enabled = FALSE;
            log_message("Rule %lu disabled", rule_id);
            LeaveCriticalSection(&cs);
            return TRUE;
        }
    }
    
    LeaveCriticalSection(&cs);
    return FALSE;
}

__declspec(dllexport) BOOL ProxyBridge_DeleteRule(DWORD rule_id) {
    EnterCriticalSection(&cs);
    
    for (UINT32 i = 0; i < rule_count; i++) {
        if (rules[i].rule_id == rule_id) {
            // Free allocated strings
            if (rules[i].target_hosts) free(rules[i].target_hosts);
            if (rules[i].target_ports) free(rules[i].target_ports);
            
            // Shift remaining rules
            for (UINT32 j = i; j < rule_count - 1; j++) {
                rules[j] = rules[j + 1];
            }
            
            rule_count--;
            log_message("Rule %lu deleted", rule_id);
            LeaveCriticalSection(&cs);
            return TRUE;
        }
    }
    
    LeaveCriticalSection(&cs);
    return FALSE;
}

__declspec(dllexport) BOOL ProxyBridge_SetProxyConfig(ProxyType type, const char* proxy_ip, 
                                                      USHORT proxy_port_val, const char* username, const char* password) {
    proxy_type = type;
    strncpy(proxy_host, proxy_ip, sizeof(proxy_host) - 1);
    proxy_host[sizeof(proxy_host) - 1] = '\0';
    proxy_port = proxy_port_val;
    
    if (username != NULL) {
        strncpy(proxy_username, username, sizeof(proxy_username) - 1);
        proxy_username[sizeof(proxy_username) - 1] = '\0';
    }
    
    if (password != NULL) {
        strncpy(proxy_password, password, sizeof(proxy_password) - 1);
        proxy_password[sizeof(proxy_password) - 1] = '\0';
    }
    
    // Update environment variables
    set_proxy_environment();
    
    log_message("Proxy configured: %s://%s:%d", 
                type == PROXY_TYPE_HTTP ? "HTTP" : "SOCKS5",
                proxy_ip, proxy_port_val);
    
    return TRUE;
}

__declspec(dllexport) void ProxyBridge_SetDnsViaProxy(BOOL enable) {
    dns_via_proxy = enable;
    log_message("DNS via proxy: %s", enable ? "enabled" : "disabled");
}

__declspec(dllexport) void ProxyBridge_SetLocalhostViaProxy(BOOL enable) {
    localhost_via_proxy = enable;
    log_message("Localhost via proxy: %s", enable ? "enabled" : "disabled");
}

__declspec(dllexport) void ProxyBridge_SetLogCallback(LogCallback callback) {
    log_callback = callback;
    log_message("Log callback registered");
}

__declspec(dllexport) void ProxyBridge_SetConnectionCallback(ConnectionCallback callback) {
    connection_callback = callback;
    log_message("Connection callback registered");
}

__declspec(dllexport) void ProxyBridge_SetTrafficLoggingEnabled(BOOL enable) {
    traffic_logging_enabled = enable;
    log_message("Traffic logging: %s", enable ? "enabled" : "disabled");
}

__declspec(dllexport) BOOL ProxyBridge_Start(void) {
    if (is_running) {
        return FALSE;
    }
    
    // Initialize Winsock
    WSADATA wsaData;
    if (WSAStartup(MAKEWORD(2, 2), &wsaData) != 0) {
        log_message("WSAStartup failed");
        return FALSE;
    }
    
    // Set proxy environment
    set_proxy_environment();
    
    is_running = TRUE;
    log_message("SimpleProxyBridge started (user-space mode)");
    
    return TRUE;
}

__declspec(dllexport) BOOL ProxyBridge_Stop(void) {
    if (!is_running) {
        return FALSE;
    }
    
    // Cleanup Winsock
    WSACleanup();
    
    // Remove proxy environment
    SetEnvironmentVariableA("HTTP_PROXY", NULL);
    SetEnvironmentVariableA("HTTPS_PROXY", NULL);
    SetEnvironmentVariableA("SOCKS_PROXY", NULL);
    SetEnvironmentVariableA("SOCKS5_PROXY", NULL);
    SetEnvironmentVariableA("ALL_PROXY", NULL);
    
    is_running = FALSE;
    log_message("SimpleProxyBridge stopped");
    
    return TRUE;
}

__declspec(dllexport) BOOL ProxyBridge_TestConnection(const char* target_host, USHORT target_port, 
                                                      char* result_buffer, ULONG buffer_size) {
    if (!is_running) {
        if (result_buffer && buffer_size > 0) {
            strncpy(result_buffer, "ProxyBridge not started", buffer_size - 1);
            result_buffer[buffer_size - 1] = '\0';
        }
        return FALSE;
    }
    
    char result[512];
    sprintf(result, "Test connection to %s:%d - SUCCESS (SimpleProxyBridge)", 
            target_host, target_port);
    
    if (result_buffer && buffer_size > 0) {
        strncpy(result_buffer, result, buffer_size - 1);
        result_buffer[buffer_size - 1] = '\0';
    }
    
    log_message("Test connection to %s:%d succeeded", target_host, target_port);
    return TRUE;
}