using System;
using System.Runtime.InteropServices;
using System.Text;

namespace ProxyBridgeCore
{
    public static class NativeMethods
    {
        private static LogCallback _logCallback;
        private static ConnectionCallback _connectionCallback;
        private static bool _isRunning = false;
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
        public static bool ProxyBridge_EnableRule(uint rule_id)
        {
            try
            {
                if (_logCallback != null)
                {
                    _logCallback($"Rule {rule_id} enabled");
                }
                return true;
            }
            catch
            {
                return false;
            }
        }

        [DllExport("ProxyBridge_DisableRule", CallingConvention = CallingConvention.Cdecl)]
        public static bool ProxyBridge_DisableRule(uint rule_id)
        {
            try
            {
                if (_logCallback != null)
                {
                    _logCallback($"Rule {rule_id} disabled");
                }
                return true;
            }
            catch
            {
                return false;
            }
        }

        [DllExport("ProxyBridge_DeleteRule", CallingConvention = CallingConvention.Cdecl)]
        public static bool ProxyBridge_DeleteRule(uint rule_id)
        {
            try
            {
                if (_logCallback != null)
                {
                    _logCallback($"Rule {rule_id} deleted");
                }
                return true;
            }
            catch
            {
                return false;
            }
        }

        [DllExport("ProxyBridge_EditRule", CallingConvention = CallingConvention.Cdecl)]
        public static bool ProxyBridge_EditRule(uint rule_id, string process_name, string target_hosts, string target_ports, RuleProtocol protocol, RuleAction action)
        {
            try
            {
                if (_logCallback != null)
                {
                    string message = $"Rule {rule_id} edited: {process_name} -> {target_hosts}:{target_ports}:{protocol} -> {action}";
                    _logCallback(message);
                }
                return true;
            }
            catch
            {
                return false;
            }
        }

        [DllExport("ProxyBridge_MoveRuleToPosition", CallingConvention = CallingConvention.Cdecl)]
        public static bool ProxyBridge_MoveRuleToPosition(uint rule_id, uint new_position)
        {
            return true;
        }

        [DllExport("ProxyBridge_GetRulePosition", CallingConvention = CallingConvention.Cdecl)]
        public static uint ProxyBridge_GetRulePosition(uint rule_id)
        {
            return 1; // Always first position
        }

        [DllExport("ProxyBridge_SetProxyConfig", CallingConvention = CallingConvention.Cdecl)]
        public static bool ProxyBridge_SetProxyConfig(ProxyType type, string proxy_ip, ushort proxy_port, string username, string password)
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
                return true;
            }
            catch
            {
                return false;
            }
        }

        [DllExport("ProxyBridge_SetDnsViaProxy", CallingConvention = CallingConvention.Cdecl)]
        public static void ProxyBridge_SetDnsViaProxy(bool enable)
        {
            try
            {
                if (_logCallback != null)
                {
                    _logCallback($"DNS via proxy: {(enable ? "enabled" : "disabled")}");
                }
            }
            catch { }
        }

        [DllExport("ProxyBridge_SetLocalhostViaProxy", CallingConvention = CallingConvention.Cdecl)]
        public static void ProxyBridge_SetLocalhostViaProxy(bool enable)
        {
            try
            {
                if (_logCallback != null)
                {
                    _logCallback($"Localhost via proxy: {(enable ? "enabled" : "disabled")}");
                }
            }
            catch { }
        }

        [DllExport("ProxyBridge_SetLogCallback", CallingConvention = CallingConvention.Cdecl)]
        public static void ProxyBridge_SetLogCallback(IntPtr callback)
        {
            try
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
            catch { }
        }

        [DllExport("ProxyBridge_SetConnectionCallback", CallingConvention = CallingConvention.Cdecl)]
        public static void ProxyBridge_SetConnectionCallback(IntPtr callback)
        {
            try
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
            catch { }
        }

        [DllExport("ProxyBridge_SetTrafficLoggingEnabled", CallingConvention = CallingConvention.Cdecl)]
        public static void ProxyBridge_SetTrafficLoggingEnabled(bool enable)
        {
            // Stub implementation
        }

        [DllExport("ProxyBridge_ClearConnectionLogs", CallingConvention = CallingConvention.Cdecl)]
        public static void ProxyBridge_ClearConnectionLogs()
        {
            // Stub implementation
        }

        [DllExport("ProxyBridge_Start", CallingConvention = CallingConvention.Cdecl)]
        public static bool ProxyBridge_Start()
        {
            try
            {
                if (_isRunning)
                {
                    return false;
                }

                if (_logCallback != null)
                {
                    _logCallback("ProxyBridge started (stub mode)");
                }

                _isRunning = true;
                return true;
            }
            catch
            {
                return false;
            }
        }

        [DllExport("ProxyBridge_Stop", CallingConvention = CallingConvention.Cdecl)]
        public static bool ProxyBridge_Stop()
        {
            try
            {
                if (!_isRunning)
                {
                    return false;
                }

                if (_logCallback != null)
                {
                    _logCallback("ProxyBridge stopped");
                }

                _isRunning = false;
                return true;
            }
            catch
            {
                return false;
            }
        }

        [DllExport("ProxyBridge_TestConnection", CallingConvention = CallingConvention.Cdecl)]
        public static int ProxyBridge_TestConnection(string target_host, ushort target_port, IntPtr result_buffer, ulong buffer_size)
        {
            try
            {
                string result = $"Test connection to {target_host}:{target_port} - SUCCESS (stub mode)";

                if (result_buffer != IntPtr.Zero && buffer_size > 0)
                {
                    byte[] resultBytes = Encoding.ASCII.GetBytes(result);
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