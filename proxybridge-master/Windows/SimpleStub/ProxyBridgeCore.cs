using System;
using System.Runtime.InteropServices;

namespace ProxyBridgeCore
{
    public static class NativeMethods
    {
        private static LogCallback _logCallback;
        private static ConnectionCallback _connectionCallback;
        private static bool _isRunning = false;
        private static bool _trafficLoggingEnabled = false;
        private static bool _dnsViaProxy = true;
        private static bool _localhostViaProxy = false;
        private static uint _nextRuleId = 1;

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

        [DllExport("ProxyBridge_AddRule", CallingConvention = CallingConvention.Cdecl)]
        public static uint ProxyBridge_AddRule(string process_name, string target_hosts, string target_ports, RuleProtocol protocol, RuleAction action)
        {
            try
            {
                if (_logCallback != null)
                {
                    string message = $"Rule added: {process_name} -> {target_hosts}:{target_ports}:{protocol} -> {action}";
                    _logCallback(message);
                }
                return _nextRuleId++;
            }
            catch (Exception ex)
            {
                LogError($"ProxyBridge_AddRule error: {ex.Message}");
                return 0;
            }
        }

        [DllExport("ProxyBridge_EnableRule", CallingConvention = CallingConvention.Cdecl)]
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

        [DllExport("ProxyBridge_DisableRule", CallingConvention = CallingConvention.Cdecl)]
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

        [DllExport("ProxyBridge_DeleteRule", CallingConvention = CallingConvention.Cdecl)]
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

        [DllExport("ProxyBridge_EditRule", CallingConvention = CallingConvention.Cdecl)]
        public static byte ProxyBridge_EditRule(uint rule_id, string process_name, string target_hosts, string target_ports, RuleProtocol protocol, RuleAction action)
        {
            try
            {
                if (_logCallback != null)
                {
                    string message = $"Rule {rule_id} edited: {process_name} -> {target_hosts}:{target_ports}:{protocol} -> {action}";
                    _logCallback(message);
                }
                return 1; // TRUE
            }
            catch
            {
                return 0; // FALSE
            }
        }

        [DllExport("ProxyBridge_MoveRuleToPosition", CallingConvention = CallingConvention.Cdecl)]
        public static byte ProxyBridge_MoveRuleToPosition(uint rule_id, uint new_position)
        {
            return 1; // TRUE
        }

        [DllExport("ProxyBridge_GetRulePosition", CallingConvention = CallingConvention.Cdecl)]
        public static uint ProxyBridge_GetRulePosition(uint rule_id)
        {
            return 1; // Always first position
        }

        [DllExport("ProxyBridge_SetProxyConfig", CallingConvention = CallingConvention.Cdecl)]
        public static byte ProxyBridge_SetProxyConfig(ProxyType type, string proxy_ip, ushort proxy_port, string username, string password)
        {
            try
            {
                string typeStr = type == ProxyType.HTTP ? "HTTP" : "SOCKS5";

                if (_logCallback != null)
                {
                    string message = $"Proxy set: {typeStr}://{proxy_ip}:{proxy_port}";
                    if (!string.IsNullOrEmpty(username))
                    {
                        message += $" (auth: {username})";
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

        [DllExport("ProxyBridge_SetDnsViaProxy", CallingConvention = CallingConvention.Cdecl)]
        public static void ProxyBridge_SetDnsViaProxy(byte enable)
        {
            _dnsViaProxy = enable != 0;
            if (_logCallback != null)
            {
                _logCallback($"DNS via proxy: {(_dnsViaProxy ? "enabled" : "disabled")}");
            }
        }

        [DllExport("ProxyBridge_SetLocalhostViaProxy", CallingConvention = CallingConvention.Cdecl)]
        public static void ProxyBridge_SetLocalhostViaProxy(byte enable)
        {
            _localhostViaProxy = enable != 0;
            if (_logCallback != null)
            {
                _logCallback($"Localhost via proxy: {(_localhostViaProxy ? "enabled" : "disabled")}");
            }
        }

        [DllExport("ProxyBridge_SetLogCallback", CallingConvention = CallingConvention.Cdecl)]
        public static void ProxyBridge_SetLogCallback(IntPtr callback)
        {
            if (callback != IntPtr.Zero)
            {
                _logCallback = Marshal.GetDelegateForFunctionPointer<LogCallback>(callback);
            }
            else
            {
                _logCallback = null;
            }
        }

        [DllExport("ProxyBridge_SetConnectionCallback", CallingConvention = CallingConvention.Cdecl)]
        public static void ProxyBridge_SetConnectionCallback(IntPtr callback)
        {
            if (callback != IntPtr.Zero)
            {
                _connectionCallback = Marshal.GetDelegateForFunctionPointer<ConnectionCallback>(callback);
            }
            else
            {
                _connectionCallback = null;
            }
        }

        [DllExport("ProxyBridge_SetTrafficLoggingEnabled", CallingConvention = CallingConvention.Cdecl)]
        public static void ProxyBridge_SetTrafficLoggingEnabled(byte enable)
        {
            _trafficLoggingEnabled = enable != 0;
        }

        [DllExport("ProxyBridge_ClearConnectionLogs", CallingConvention = CallingConvention.Cdecl)]
        public static void ProxyBridge_ClearConnectionLogs()
        {
            // Stub
        }

        [DllExport("ProxyBridge_Start", CallingConvention = CallingConvention.Cdecl)]
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

        [DllExport("ProxyBridge_Stop", CallingConvention = CallingConvention.Cdecl)]
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

        [DllExport("ProxyBridge_TestConnection", CallingConvention = CallingConvention.Cdecl)]
        public static int ProxyBridge_TestConnection(string target_host, ushort target_port, IntPtr result_buffer, ulong buffer_size)
        {
            try
            {
                string result = $"Test connection to {target_host}:{target_port} - SUCCESS (stub mode)";

                if (result_buffer != IntPtr.Zero && buffer_size > 0)
                {
                    byte[] resultBytes = System.Text.Encoding.ASCII.GetBytes(result);
                    int copyLength = (int)Math.Min(buffer_size - 1, (ulong)resultBytes.Length);
                    Marshal.Copy(resultBytes, 0, result_buffer, copyLength);
                    Marshal.WriteByte(result_buffer + copyLength, 0); // Null terminator
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

    [AttributeUsage(AttributeTargets.Method)]
    public class DllExportAttribute : Attribute
    {
        public string EntryPoint { get; }
        public CallingConvention CallingConvention { get; }

        public DllExportAttribute(string entryPoint, CallingConvention callingConvention = CallingConvention.Cdecl)
        {
            EntryPoint = entryPoint;
            CallingConvention = callingConvention;
        }
    }
}