using System;
using System.Runtime.InteropServices;
using System.Text;

namespace ProxyBridgeCore
{
    [UnmanagedFunctionPointer(CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public delegate void LogCallback(string message);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public delegate void ConnectionCallback(string processName, uint pid, string destIp, ushort destPort, string proxyInfo);

    public enum ProxyType
    {
        HTTP = 0,
        SOCKS5 = 1
    }

    public enum RuleAction
    {
        PROXY = 0,
        DIRECT = 1,
        BLOCK = 2
    }

    public enum RuleProtocol
    {
        TCP = 0,
        UDP = 1,
        BOTH = 2
    }

    public static class NativeMethods
    {
        private static LogCallback _logCallback;
        private static ConnectionCallback _connectionCallback;
        private static bool _isRunning = false;
        private static bool _trafficLoggingEnabled = false;
        private static bool _dnsViaProxy = true;
        private static bool _localhostViaProxy = false;
        private static uint _nextRuleId = 1;

        [UnmanagedCallersOnly(EntryPoint = "ProxyBridge_AddRule", CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        public static uint ProxyBridge_AddRule(byte* process_name, byte* target_hosts, byte* target_ports, RuleProtocol protocol, RuleAction action)
        {
            try
            {
                string processName = Marshal.PtrToStringAnsi((IntPtr)process_name) ?? string.Empty;
                string targetHosts = Marshal.PtrToStringAnsi((IntPtr)target_hosts) ?? string.Empty;
                string targetPorts = Marshal.PtrToStringAnsi((IntPtr)target_ports) ?? string.Empty;

                if (_logCallback != null)
                {
                    string message = $"Rule added: {processName} -> {targetHosts}:{targetPorts}:{protocol} -> {action}";
                    _logCallback(message);
                }

                // Return dummy rule ID
                return _nextRuleId++;
            }
            catch (Exception ex)
            {
                LogError($"ProxyBridge_AddRule error: {ex.Message}");
                return 0;
            }
        }

        [UnmanagedCallersOnly(EntryPoint = "ProxyBridge_EnableRule", CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        public static byte ProxyBridge_EnableRule(uint rule_id)
        {
            try
            {
                if (_logCallback != null)
                {
                    _logCallback($"Rule {rule_id} enabled");
                }
                return 1; // TRUE
            }
            catch
            {
                return 0; // FALSE
            }
        }

        [UnmanagedCallersOnly(EntryPoint = "ProxyBridge_DisableRule", CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        public static byte ProxyBridge_DisableRule(uint rule_id)
        {
            try
            {
                if (_logCallback != null)
                {
                    _logCallback($"Rule {rule_id} disabled");
                }
                return 1; // TRUE
            }
            catch
            {
                return 0; // FALSE
            }
        }

        [UnmanagedCallersOnly(EntryPoint = "ProxyBridge_DeleteRule", CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        public static byte ProxyBridge_DeleteRule(uint rule_id)
        {
            try
            {
                if (_logCallback != null)
                {
                    _logCallback($"Rule {rule_id} deleted");
                }
                return 1; // TRUE
            }
            catch
            {
                return 0; // FALSE
            }
        }

        [UnmanagedCallersOnly(EntryPoint = "ProxyBridge_EditRule", CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        public static byte ProxyBridge_EditRule(uint rule_id, byte* process_name, byte* target_hosts, byte* target_ports, RuleProtocol protocol, RuleAction action)
        {
            try
            {
                string processName = Marshal.PtrToStringAnsi((IntPtr)process_name) ?? string.Empty;
                string targetHosts = Marshal.PtrToStringAnsi((IntPtr)target_hosts) ?? string.Empty;
                string targetPorts = Marshal.PtrToStringAnsi((IntPtr)target_ports) ?? string.Empty;

                if (_logCallback != null)
                {
                    string message = $"Rule {rule_id} edited: {processName} -> {targetHosts}:{targetPorts}:{protocol} -> {action}";
                    _logCallback(message);
                }
                return 1; // TRUE
            }
            catch
            {
                return 0; // FALSE
            }
        }

        [UnmanagedCallersOnly(EntryPoint = "ProxyBridge_MoveRuleToPosition", CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        public static byte ProxyBridge_MoveRuleToPosition(uint rule_id, uint new_position)
        {
            return 1; // TRUE
        }

        [UnmanagedCallersOnly(EntryPoint = "ProxyBridge_GetRulePosition", CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        public static uint ProxyBridge_GetRulePosition(uint rule_id)
        {
            return 1; // Always first position
        }

        [UnmanagedCallersOnly(EntryPoint = "ProxyBridge_SetProxyConfig", CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        public static byte ProxyBridge_SetProxyConfig(ProxyType type, byte* proxy_ip, ushort proxy_port, byte* username, byte* password)
        {
            try
            {
                string proxyIp = Marshal.PtrToStringAnsi((IntPtr)proxy_ip) ?? string.Empty;
                string userName = Marshal.PtrToStringAnsi((IntPtr)username) ?? string.Empty;
                string typeStr = type == ProxyType.HTTP ? "HTTP" : "SOCKS5";

                if (_logCallback != null)
                {
                    string message = $"Proxy set: {typeStr}://{proxyIp}:{proxy_port}";
                    if (!string.IsNullOrEmpty(userName))
                    {
                        message += $" (auth: {userName})";
                    }
                    _logCallback(message);
                }
                return 1; // TRUE
            }
            catch
            {
                return 0; // FALSE
            }
        }

        [UnmanagedCallersOnly(EntryPoint = "ProxyBridge_SetDnsViaProxy", CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        public static void ProxyBridge_SetDnsViaProxy(byte enable)
        {
            _dnsViaProxy = enable != 0;
            if (_logCallback != null)
            {
                _logCallback($"DNS via proxy: {(_dnsViaProxy ? "enabled" : "disabled")}");
            }
        }

        [UnmanagedCallersOnly(EntryPoint = "ProxyBridge_SetLocalhostViaProxy", CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        public static void ProxyBridge_SetLocalhostViaProxy(byte enable)
        {
            _localhostViaProxy = enable != 0;
            if (_logCallback != null)
            {
                _logCallback($"Localhost via proxy: {(_localhostViaProxy ? "enabled" : "disabled")}");
            }
        }

        [UnmanagedCallersOnly(EntryPoint = "ProxyBridge_SetLogCallback", CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        public static void ProxyBridge_SetLogCallback(nint callback)
        {
            if (callback != 0)
            {
                _logCallback = Marshal.GetDelegateForFunctionPointer<LogCallback>((IntPtr)callback);
            }
            else
            {
                _logCallback = null;
            }
        }

        [UnmanagedCallersOnly(EntryPoint = "ProxyBridge_SetConnectionCallback", CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        public static void ProxyBridge_SetConnectionCallback(nint callback)
        {
            if (callback != 0)
            {
                _connectionCallback = Marshal.GetDelegateForFunctionPointer<ConnectionCallback>((IntPtr)callback);
            }
            else
            {
                _connectionCallback = null;
            }
        }

        [UnmanagedCallersOnly(EntryPoint = "ProxyBridge_SetTrafficLoggingEnabled", CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        public static void ProxyBridge_SetTrafficLoggingEnabled(byte enable)
        {
            _trafficLoggingEnabled = enable != 0;
        }

        [UnmanagedCallersOnly(EntryPoint = "ProxyBridge_ClearConnectionLogs", CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        public static void ProxyBridge_ClearConnectionLogs()
        {
            // Stub
        }

        [UnmanagedCallersOnly(EntryPoint = "ProxyBridge_Start", CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        public static byte ProxyBridge_Start()
        {
            if (_isRunning)
            {
                return 0; // FALSE
            }

            if (_logCallback != null)
            {
                _logCallback("ProxyBridge started (stub mode)");
            }

            _isRunning = true;
            return 1; // TRUE
        }

        [UnmanagedCallersOnly(EntryPoint = "ProxyBridge_Stop", CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        public static byte ProxyBridge_Stop()
        {
            if (!_isRunning)
            {
                return 0; // FALSE
            }

            if (_logCallback != null)
            {
                _logCallback("ProxyBridge stopped");
            }

            _isRunning = false;
            return 1; // TRUE
        }

        [UnmanagedCallersOnly(EntryPoint = "ProxyBridge_TestConnection", CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        public static int ProxyBridge_TestConnection(byte* target_host, ushort target_port, byte* result_buffer, nint buffer_size)
        {
            try
            {
                string targetHost = Marshal.PtrToStringAnsi((IntPtr)target_host) ?? string.Empty;
                string result = $"Test connection to {targetHost}:{target_port} - SUCCESS (stub mode)";

                if (result_buffer != null && buffer_size > 0)
                {
                    byte[] resultBytes = Encoding.ASCII.GetBytes(result);
                    int copyLength = Math.Min((int)buffer_size - 1, resultBytes.Length);
                    Marshal.Copy(resultBytes, 0, (IntPtr)result_buffer, copyLength);
                    Marshal.WriteByte((IntPtr)result_buffer + copyLength, 0); // Null terminator
                }

                return 0; // Success
            }
            catch
            {
                return 1; // Error
            }
        }

        private static void LogError(string message)
        {
            try
            {
                System.IO.File.AppendAllText("ProxyBridgeCore_stub.log", $"{DateTime.Now}: {message}\n");
            }
            catch { }
        }
    }
}