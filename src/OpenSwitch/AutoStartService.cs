using Microsoft.Win32;
using System.Diagnostics;

namespace OpenSwitch;

public sealed class AutoStartService
{
    private const string RunKeyPath = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string ValueName = "OpenSwitch";

    public bool IsEnabled()
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKeyPath, writable: false);
        return key?.GetValue(ValueName) is string;
    }

    public void SetEnabled(bool enabled)
    {
        using var key = Registry.CurrentUser.CreateSubKey(RunKeyPath, writable: true)
            ?? throw new InvalidOperationException("Не удалось открыть раздел автозапуска.");

        if (enabled)
        {
            var executablePath = GetExecutablePath();
            key.SetValue(ValueName, $"\"{executablePath}\" --startup", RegistryValueKind.String);
        }
        else
        {
            key.DeleteValue(ValueName, throwOnMissingValue: false);
        }
    }

    private static string GetExecutablePath()
    {
        var applicationPath = Path.Combine(AppContext.BaseDirectory, "OpenSwitch.exe");
        if (File.Exists(applicationPath))
        {
            return applicationPath;
        }

        return Environment.ProcessPath
            ?? Process.GetCurrentProcess().MainModule?.FileName
            ?? Application.ExecutablePath;
    }
}
