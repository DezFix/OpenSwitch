using System.ComponentModel;
using System.Runtime.InteropServices;

namespace OpenSwitch;

internal static class NativeMethods
{
    internal const int WM_HOTKEY = 0x0312;
    internal const int WM_INPUTLANGCHANGEREQUEST = 0x0010;
    internal const uint MOD_ALT = 0x0001;
    internal const uint MOD_CONTROL = 0x0002;
    internal const uint MOD_SHIFT = 0x0004;
    internal const uint MOD_WIN = 0x0008;
    internal const uint MOD_NOREPEAT = 0x4000;
    internal const ushort VK_CONTROL = 0x11;
    internal const ushort VK_SHIFT = 0x10;
    internal const ushort VK_C = 0x43;
    internal const ushort VK_V = 0x56;
    internal const ushort VK_LEFT = 0x25;
    internal const ushort VK_SCROLL = 0x91;

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static extern bool RegisterHotKey(IntPtr hWnd, int id, uint fsModifiers, uint vk);

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static extern bool UnregisterHotKey(IntPtr hWnd, int id);

    [DllImport("user32.dll")]
    internal static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static extern bool PostMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll", SetLastError = true)]
    internal static extern uint SendInput(uint nInputs, INPUT[] pInputs, int cbSize);

    [DllImport("user32.dll")]
    internal static extern uint GetClipboardSequenceNumber();

    internal static void SendKeyChord(params ushort[] virtualKeys)
    {
        var inputs = new INPUT[virtualKeys.Length * 2];
        var inputIndex = 0;

        foreach (var virtualKey in virtualKeys)
        {
            inputs[inputIndex++] = CreateKeyInput(virtualKey, keyUp: false);
        }

        for (var index = virtualKeys.Length - 1; index >= 0; index--)
        {
            inputs[inputIndex++] = CreateKeyInput(virtualKeys[index], keyUp: true);
        }

        var sentInputs = SendInput((uint)inputs.Length, inputs, Marshal.SizeOf<INPUT>());
        if (sentInputs != inputs.Length)
        {
            throw new Win32Exception(Marshal.GetLastWin32Error());
        }
    }

    private static INPUT CreateKeyInput(ushort virtualKey, bool keyUp)
    {
        return new INPUT
        {
            Type = 1,
            Union = new InputUnion
            {
                Keyboard = new KeyboardInput
                {
                    VirtualKey = virtualKey,
                    Flags = keyUp ? 0x0002u : 0u
                }
            }
        };
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct INPUT
    {
        public uint Type;
        public InputUnion Union;
    }

    [StructLayout(LayoutKind.Explicit)]
    internal struct InputUnion
    {
        [FieldOffset(0)]
        public KeyboardInput Keyboard;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct KeyboardInput
    {
        public ushort VirtualKey;
        public ushort ScanCode;
        public uint Flags;
        public uint Time;
        public UIntPtr ExtraInfo;
    }
}
