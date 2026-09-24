using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

namespace OpenSwitch;

public static class ProgramRuleMatcher
{
    public static bool IsExcluded(AppSettings settings)
    {
        if (settings.DoNotInteractWithProgramRules)
        {
            return true;
        }

        var foregroundWindow = NativeMethods.GetForegroundWindow();
        if (foregroundWindow == IntPtr.Zero)
        {
            return false;
        }

        var titleLength = GetWindowTextLength(foregroundWindow);
        var titleBuilder = new StringBuilder(titleLength + 1);
        GetWindowText(foregroundWindow, titleBuilder, titleBuilder.Capacity);
        var title = titleBuilder.ToString();
        GetWindowThreadProcessId(foregroundWindow, out var processId);

        try
        {
            using var process = Process.GetProcessById((int)processId);
            var processName = process.ProcessName;
            string? processPath = null;
            try
            {
                processPath = process.MainModule?.FileName;
            }
            catch
            {
            }

            return settings.ProgramRules.Any(rule =>
                rule.Enabled
                && (Matches(rule.ProcessName, processName)
                    || Matches(rule.Path, processPath)
                    || Matches(rule.WindowTitle, title)));
        }
        catch (ArgumentException)
        {
            return false;
        }
    }

    private static bool Matches(string configuredValue, string? actualValue)
    {
        return !string.IsNullOrWhiteSpace(configuredValue)
            && !string.IsNullOrWhiteSpace(actualValue)
            && actualValue.Contains(configuredValue, StringComparison.OrdinalIgnoreCase);
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowTextLength(IntPtr hWnd);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int maxCount);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
