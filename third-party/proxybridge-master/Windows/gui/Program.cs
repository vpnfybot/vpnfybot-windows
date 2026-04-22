using Avalonia;
using System;
using System.Threading;
using System.Runtime.InteropServices;

namespace ProxyBridge.GUI;

class Program
{
    private static Mutex? _instanceMutex;
    private const string MutexName = "Global\\ProxyBridge_SingleInstance_Mutex_v3.1";
    private const string EventName = "Global\\ProxyBridge_ShowWindow_Event_v3.1";

    // Windows 11 titlebar dark mode support
    private const int DWMWA_USE_IMMERSIVE_DARK_MODE = 20;

    [DllImport("dwmapi.dll", PreserveSig = true)]
    private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attr, ref int attrValue, int attrSize);

    [STAThread]
    public static void Main(string[] args)
    {
        _instanceMutex = new Mutex(true, MutexName, out bool isNewInstance);

        if (!isNewInstance)
        {
            SignalExistingInstance();
            return;
        }

        try
        {
            App.StartMinimized = args.Length > 0 && args[0] == "--minimized";
            BuildAvaloniaApp().StartWithClassicDesktopLifetime(args);
        }
        finally
        {
            _instanceMutex?.ReleaseMutex();
            _instanceMutex?.Dispose();
        }
    }

    private static void SignalExistingInstance()
    {
        try
        {
            using var showEvent = EventWaitHandle.OpenExisting(EventName);
            showEvent.Set();
        }
        catch { }
    }

    public static AppBuilder BuildAvaloniaApp()
        => AppBuilder.Configure<App>()
            .UsePlatformDetect()
            .WithInterFont()
            .LogToTrace();
}
