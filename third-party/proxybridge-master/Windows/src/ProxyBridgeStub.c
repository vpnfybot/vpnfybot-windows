#include <windows.h>
#include "ProxyBridge.h"

#ifdef PROXYBRIDGE_EXPORTS
#define PROXYBRIDGE_API __declspec(dllexport)
#else
#define PROXYBRIDGE_API __declspec(dllimport)
#endif

#ifdef __cplusplus
extern "C" {
#endif

// Глобальные переменные состояния
static BOOL g_isRunning = FALSE;
static LogCallback g_logCallback = NULL;
static ConnectionCallback g_connectionCallback = NULL;
static BOOL g_trafficLoggingEnabled = FALSE;
static BOOL g_dnsViaProxy = TRUE;
static BOOL g_localhostViaProxy = FALSE;

PROXYBRIDGE_API UINT32 ProxyBridge_AddRule(const char* process_name, const char* target_hosts, const char* target_ports, RuleProtocol protocol, RuleAction action)
{
    if (g_logCallback != NULL)
    {
        char buffer[256];
        snprintf(buffer, sizeof(buffer), "Rule added: %s -> %s:%s:%d -> %d", 
                 process_name, target_hosts, target_ports, protocol, action);
        g_logCallback(buffer);
    }
    
    // Возвращаем фиктивный ID правила
    static UINT32 nextRuleId = 1;
    return nextRuleId++;
}

PROXYBRIDGE_API BOOL ProxyBridge_EnableRule(UINT32 rule_id)
{
    if (g_logCallback != NULL)
    {
        char buffer[128];
        snprintf(buffer, sizeof(buffer), "Rule %u enabled", rule_id);
        g_logCallback(buffer);
    }
    return TRUE;
}

PROXYBRIDGE_API BOOL ProxyBridge_DisableRule(UINT32 rule_id)
{
    if (g_logCallback != NULL)
    {
        char buffer[128];
        snprintf(buffer, sizeof(buffer), "Rule %u disabled", rule_id);
        g_logCallback(buffer);
    }
    return TRUE;
}

PROXYBRIDGE_API BOOL ProxyBridge_DeleteRule(UINT32 rule_id)
{
    if (g_logCallback != NULL)
    {
        char buffer[128];
        snprintf(buffer, sizeof(buffer), "Rule %u deleted", rule_id);
        g_logCallback(buffer);
    }
    return TRUE;
}

PROXYBRIDGE_API BOOL ProxyBridge_EditRule(UINT32 rule_id, const char* process_name, const char* target_hosts, const char* target_ports, RuleProtocol protocol, RuleAction action)
{
    if (g_logCallback != NULL)
    {
        char buffer[256];
        snprintf(buffer, sizeof(buffer), "Rule %u edited: %s -> %s:%s:%d -> %d", 
                 rule_id, process_name, target_hosts, target_ports, protocol, action);
        g_logCallback(buffer);
    }
    return TRUE;
}

PROXYBRIDGE_API BOOL ProxyBridge_MoveRuleToPosition(UINT32 rule_id, UINT32 new_position)
{
    return TRUE;
}

PROXYBRIDGE_API UINT32 ProxyBridge_GetRulePosition(UINT32 rule_id)
{
    return 1;
}

PROXYBRIDGE_API BOOL ProxyBridge_SetProxyConfig(ProxyType type, const char* proxy_ip, UINT16 proxy_port, const char* username, const char* password)
{
    if (g_logCallback != NULL)
    {
        char buffer[256];
        const char* typeStr = (type == PROXY_TYPE_HTTP) ? "HTTP" : "SOCKS5";
        snprintf(buffer, sizeof(buffer), "Proxy set: %s://%s:%u", typeStr, proxy_ip, proxy_port);
        if (username != NULL && username[0] != '\0')
        {
            size_t len = strlen(buffer);
            snprintf(buffer + len, sizeof(buffer) - len, " (auth: %s)", username);
        }
        g_logCallback(buffer);
    }
    return TRUE;
}

PROXYBRIDGE_API void ProxyBridge_SetDnsViaProxy(BOOL enable)
{
    g_dnsViaProxy = enable;
    if (g_logCallback != NULL)
    {
        char buffer[128];
        snprintf(buffer, sizeof(buffer), "DNS via proxy: %s", enable ? "enabled" : "disabled");
        g_logCallback(buffer);
    }
}

PROXYBRIDGE_API void ProxyBridge_SetLocalhostViaProxy(BOOL enable)
{
    g_localhostViaProxy = enable;
    if (g_logCallback != NULL)
    {
        char buffer[128];
        snprintf(buffer, sizeof(buffer), "Localhost via proxy: %s", enable ? "enabled" : "disabled");
        g_logCallback(buffer);
    }
}

PROXYBRIDGE_API void ProxyBridge_SetLogCallback(LogCallback callback)
{
    g_logCallback = callback;
}

PROXYBRIDGE_API void ProxyBridge_SetConnectionCallback(ConnectionCallback callback)
{
    g_connectionCallback = callback;
}

PROXYBRIDGE_API void ProxyBridge_SetTrafficLoggingEnabled(BOOL enable)
{
    g_trafficLoggingEnabled = enable;
}

PROXYBRIDGE_API void ProxyBridge_ClearConnectionLogs(void)
{
    // Заглушка
}

PROXYBRIDGE_API BOOL ProxyBridge_Start(void)
{
    if (g_isRunning)
    {
        return FALSE;
    }
    
    if (g_logCallback != NULL)
    {
        g_logCallback("ProxyBridge started (stub mode)");
    }
    
    g_isRunning = TRUE;
    return TRUE;
}

PROXYBRIDGE_API BOOL ProxyBridge_Stop(void)
{
    if (!g_isRunning)
    {
        return FALSE;
    }
    
    if (g_logCallback != NULL)
    {
        g_logCallback("ProxyBridge stopped");
    }
    
    g_isRunning = FALSE;
    return TRUE;
}

PROXYBRIDGE_API int ProxyBridge_TestConnection(const char* target_host, UINT16 target_port, char* result_buffer, size_t buffer_size)
{
    // Всегда успешно для заглушки
    if (result_buffer != NULL && buffer_size > 0)
    {
        snprintf(result_buffer, buffer_size, "Test connection to %s:%u - SUCCESS (stub mode)", target_host, target_port);
    }
    return 0; // 0 = успех
}

#ifdef __cplusplus
}
#endif