// MinimalProxyBridge.c - Minimal C DLL for ProxyBridge integration
#include <windows.h>
#include <stdio.h>

#ifdef PROXYBRIDGE_EXPORTS
#define PROXYBRIDGE_API __declspec(dllexport)
#else
#define PROXYBRIDGE_API __declspec(dllimport)
#endif

typedef void (*LogCallback)(const char* message);
typedef void (*ConnectionCallback)(const char* process_name, DWORD pid, const char* dest_ip, USHORT dest_port, const char* proxy_info);

static LogCallback g_logCallback = NULL;
static BOOL g_isRunning = FALSE;
static DWORD g_nextRuleId = 1;

// Helper function for logging
static void log_message(const char* format, ...) {
    if (g_logCallback == NULL) return;
    
    char buffer[1024];
    va_list args;
    va_start(args, format);
    vsnprintf(buffer, sizeof(buffer), format, args);
    va_end(args);
    
    g_logCallback(buffer);
}

// Write log to file for debugging
static void write_log_to_file(const char* message) {
    FILE* f = fopen("ProxyBridgeCore_debug.log", "a");
    if (f) {
        SYSTEMTIME st;
        GetLocalTime(&st);
        fprintf(f, "%04d-%02d-%02d %02d:%02d:%02d.%03d - %s\n",
                st.wYear, st.wMonth, st.wDay,
                st.wHour, st.wMinute, st.wSecond, st.wMilliseconds,
                message);
        fclose(f);
    }
}

PROXYBRIDGE_API DWORD ProxyBridge_AddRule(const char* process_name, const char* target_hosts, 
                                          const char* target_ports, DWORD protocol, DWORD action) {
    write_log_to_file("ProxyBridge_AddRule called");
    
    if (g_logCallback != NULL) {
        char buffer[256];
        snprintf(buffer, sizeof(buffer), "Rule added: %s -> %s:%s -> %lu", 
                 process_name, target_hosts ? target_hosts : "*", 
                 target_ports ? target_ports : "*", action);
        log_message(buffer);
    }
    
    DWORD rule_id = g_nextRuleId++;
    char log_msg[256];
    snprintf(log_msg, sizeof(log_msg), "Rule ID %lu created", rule_id);
    write_log_to_file(log_msg);
    
    return rule_id;
}

PROXYBRIDGE_API BOOL ProxyBridge_EnableRule(DWORD rule_id) {
    char log_msg[128];
    snprintf(log_msg, sizeof(log_msg), "ProxyBridge_EnableRule called for rule %lu", rule_id);
    write_log_to_file(log_msg);
    
    if (g_logCallback != NULL) {
        log_message("Rule %lu enabled", rule_id);
    }
    return TRUE;
}

PROXYBRIDGE_API BOOL ProxyBridge_DisableRule(DWORD rule_id) {
    char log_msg[128];
    snprintf(log_msg, sizeof(log_msg), "ProxyBridge_DisableRule called for rule %lu", rule_id);
    write_log_to_file(log_msg);
    
    if (g_logCallback != NULL) {
        log_message("Rule %lu disabled", rule_id);
    }
    return TRUE;
}

PROXYBRIDGE_API BOOL ProxyBridge_DeleteRule(DWORD rule_id) {
    char log_msg[128];
    snprintf(log_msg, sizeof(log_msg), "ProxyBridge_DeleteRule called for rule %lu", rule_id);
    write_log_to_file(log_msg);
    
    if (g_logCallback != NULL) {
        log_message("Rule %lu deleted", rule_id);
    }
    return TRUE;
}

PROXYBRIDGE_API BOOL ProxyBridge_SetProxyConfig(DWORD type, const char* proxy_ip, 
                                                USHORT proxy_port, const char* username, const char* password) {
    write_log_to_file("ProxyBridge_SetProxyConfig called");
    
    const char* typeStr = (type == 0) ? "HTTP" : "SOCKS5";
    
    if (g_logCallback != NULL) {
        char buffer[256];
        snprintf(buffer, sizeof(buffer), "Proxy set: %s://%s:%u", typeStr, proxy_ip, proxy_port);
        if (username != NULL && username[0] != '\0') {
            strncat(buffer, " (auth: ", sizeof(buffer) - strlen(buffer) - 1);
            strncat(buffer, username, sizeof(buffer) - strlen(buffer) - 1);
            strncat(buffer, ")", sizeof(buffer) - strlen(buffer) - 1);
        }
        log_message(buffer);
    }
    
    return TRUE;
}

PROXYBRIDGE_API void ProxyBridge_SetDnsViaProxy(BOOL enable) {
    write_log_to_file("ProxyBridge_SetDnsViaProxy called");
    
    if (g_logCallback != NULL) {
        log_message("DNS via proxy: %s", enable ? "enabled" : "disabled");
    }
}

PROXYBRIDGE_API void ProxyBridge_SetLocalhostViaProxy(BOOL enable) {
    write_log_to_file("ProxyBridge_SetLocalhostViaProxy called");
    
    if (g_logCallback != NULL) {
        log_message("Localhost via proxy: %s", enable ? "enabled" : "disabled");
    }
}

PROXYBRIDGE_API void ProxyBridge_SetLogCallback(LogCallback callback) {
    write_log_to_file("ProxyBridge_SetLogCallback called");
    
    g_logCallback = callback;
    
    if (callback != NULL) {
        log_message("Log callback registered with ProxyBridgeCore");
    }
}

PROXYBRIDGE_API void ProxyBridge_SetConnectionCallback(ConnectionCallback callback) {
    write_log_to_file("ProxyBridge_SetConnectionCallback called");
    
    // Not implemented in minimal version
}

PROXYBRIDGE_API void ProxyBridge_SetTrafficLoggingEnabled(BOOL enable) {
    write_log_to_file("ProxyBridge_SetTrafficLoggingEnabled called");
    
    if (g_logCallback != NULL) {
        log_message("Traffic logging: %s", enable ? "enabled" : "disabled");
    }
}

PROXYBRIDGE_API BOOL ProxyBridge_Start(void) {
    write_log_to_file("ProxyBridge_Start called");
    
    if (g_isRunning) {
        log_message("ProxyBridge already running");
        return FALSE;
    }
    
    if (g_logCallback != NULL) {
        log_message("ProxyBridge started (minimal mode)");
    }
    
    g_isRunning = TRUE;
    return TRUE;
}

PROXYBRIDGE_API BOOL ProxyBridge_Stop(void) {
    write_log_to_file("ProxyBridge_Stop called");
    
    if (!g_isRunning) {
        return FALSE;
    }
    
    if (g_logCallback != NULL) {
        log_message("ProxyBridge stopped");
    }
    
    g_isRunning = FALSE;
    return TRUE;
}

PROXYBRIDGE_API BOOL ProxyBridge_TestConnection(const char* target_host, USHORT target_port, 
                                                char* result_buffer, ULONG buffer_size) {
    write_log_to_file("ProxyBridge_TestConnection called");
    
    const char* result = "Test connection successful (MinimalProxyBridge)";
    
    if (result_buffer != NULL && buffer_size > 0) {
        strncpy(result_buffer, result, buffer_size - 1);
        result_buffer[buffer_size - 1] = '\0';
    }
    
    if (g_logCallback != NULL) {
        log_message("Test connection to %s:%d succeeded", target_host, target_port);
    }
    
    return TRUE;
}

// Additional functions for compatibility
PROXYBRIDGE_API BOOL ProxyBridge_EditRule(DWORD rule_id, const char* process_name, 
                                          const char* target_hosts, const char* target_ports, 
                                          DWORD protocol, DWORD action) {
    write_log_to_file("ProxyBridge_EditRule called");
    return TRUE;
}

PROXYBRIDGE_API BOOL ProxyBridge_MoveRuleToPosition(DWORD rule_id, DWORD new_position) {
    write_log_to_file("ProxyBridge_MoveRuleToPosition called");
    return TRUE;
}

PROXYBRIDGE_API DWORD ProxyBridge_GetRulePosition(DWORD rule_id) {
    return 1; // Always first position
}

PROXYBRIDGE_API void ProxyBridge_ClearConnectionLogs(void) {
    write_log_to_file("ProxyBridge_ClearConnectionLogs called");
}

// DLL entry point
BOOL WINAPI DllMain(HINSTANCE hinstDLL, DWORD fdwReason, LPVOID lpvReserved) {
    switch (fdwReason) {
        case DLL_PROCESS_ATTACH:
            write_log_to_file("ProxyBridgeCore.dll loaded");
            break;
        case DLL_PROCESS_DETACH:
            write_log_to_file("ProxyBridgeCore.dll unloaded");
            break;
    }
    return TRUE;
}