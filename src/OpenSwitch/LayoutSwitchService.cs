namespace OpenSwitch;

public static class LayoutSwitchService
{
    public static bool Toggle()
    {
        var foregroundWindow = NativeMethods.GetForegroundWindow();
        return foregroundWindow != IntPtr.Zero
            && NativeMethods.PostMessage(
                foregroundWindow,
                NativeMethods.WM_INPUTLANGCHANGEREQUEST,
                IntPtr.Zero,
                IntPtr.Zero);
    }
}
