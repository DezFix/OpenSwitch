using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace OpenSwitch;

public enum TextTransformMode
{
    Layout,
    Transliterate
}

public sealed class TextTransformer
{
    private string? swapBuffer;

    public string? LastSourceText { get; private set; }

    public bool TryTransform(bool currentWord, out string error)
    {
        return TryTransform(currentWord, TextTransformMode.Layout, out error);
    }

    public bool TryTransform(bool currentWord, TextTransformMode mode, out string error)
    {
        error = string.Empty;
        LastSourceText = null;
        var targetWindow = NativeMethods.GetForegroundWindow();
        if (targetWindow == IntPtr.Zero)
        {
            error = "Не удалось определить активное окно.";
            return false;
        }

        IDataObject? previousClipboard = null;
        try
        {
            previousClipboard = Clipboard.GetDataObject();
        }
        catch (ExternalException)
        {
        }

        try
        {
            if (currentWord)
            {
                NativeMethods.SendKeyChord(
                    NativeMethods.VK_CONTROL,
                    NativeMethods.VK_SHIFT,
                    NativeMethods.VK_LEFT);
            }

            var clipboardSequence = NativeMethods.GetClipboardSequenceNumber();
            NativeMethods.SendKeyChord(NativeMethods.VK_CONTROL, NativeMethods.VK_C);
            var sourceText = ReadClipboardText(clipboardSequence);
            if (sourceText is null)
            {
                error = "Не удалось получить выделенный текст.";
                return false;
            }

            LastSourceText = sourceText;
            var convertedText = mode == TextTransformMode.Layout
                ? LayoutConverter.Convert(sourceText).Text
                : sourceText.Any(IsCyrillic)
                    ? Transliterator.ToLatin(sourceText)
                    : Transliterator.ToRussian(sourceText);
            Clipboard.SetText(convertedText);
            NativeMethods.SendKeyChord(NativeMethods.VK_CONTROL, NativeMethods.VK_V);
            Thread.Sleep(20);

            if (mode == TextTransformMode.Layout
                && !NativeMethods.PostMessage(
                    targetWindow,
                    NativeMethods.WM_INPUTLANGCHANGEREQUEST,
                    IntPtr.Zero,
                    IntPtr.Zero))
            {
                error = "Текст изменён, но раскладку переключить не удалось.";
                return false;
            }

            return true;
        }
        catch (Exception exception) when (exception is Win32Exception or ExternalException or InvalidOperationException)
        {
            error = exception.Message;
            return false;
        }
        finally
        {
            RestoreClipboard(previousClipboard);
        }
    }

    public bool TryReplaceCurrentWord(string replacement, out string error)
    {
        error = string.Empty;
        LastSourceText = null;
        var targetWindow = NativeMethods.GetForegroundWindow();
        if (targetWindow == IntPtr.Zero)
        {
            error = "Не удалось определить активное окно.";
            return false;
        }

        IDataObject? previousClipboard = null;
        try
        {
            previousClipboard = Clipboard.GetDataObject();
        }
        catch (ExternalException)
        {
        }

        try
        {
            NativeMethods.SendKeyChord(
                NativeMethods.VK_CONTROL,
                NativeMethods.VK_SHIFT,
                NativeMethods.VK_LEFT);
            Clipboard.SetText(replacement);
            NativeMethods.SendKeyChord(NativeMethods.VK_CONTROL, NativeMethods.VK_V);
            return true;
        }
        catch (Exception exception) when (exception is Win32Exception or ExternalException or InvalidOperationException)
        {
            error = exception.Message;
            return false;
        }
        finally
        {
            RestoreClipboard(previousClipboard);
        }
    }

    public bool TrySwapClipboard(out string error)
    {
        error = string.Empty;
        try
        {
            if (!Clipboard.ContainsText(TextDataFormat.UnicodeText))
            {
                error = "В буфере обмена нет текста.";
                return false;
            }

            var currentText = Clipboard.GetText(TextDataFormat.UnicodeText);
            if (swapBuffer is null)
            {
                swapBuffer = currentText;
            }
            else
            {
                var nextText = swapBuffer;
                swapBuffer = currentText;
                Clipboard.SetText(nextText);
            }

            LastSourceText = currentText;
            return true;
        }
        catch (ExternalException exception)
        {
            error = exception.Message;
            return false;
        }
    }

    public bool TryTransformClipboard(TextTransformMode mode, out string error)
    {
        error = string.Empty;
        try
        {
            if (!Clipboard.ContainsText(TextDataFormat.UnicodeText))
            {
                error = "В буфере обмена нет текста.";
                return false;
            }

            var sourceText = Clipboard.GetText(TextDataFormat.UnicodeText);
            LastSourceText = sourceText;
            var convertedText = mode == TextTransformMode.Layout
                ? LayoutConverter.Convert(sourceText).Text
                : sourceText.Any(IsCyrillic)
                    ? Transliterator.ToLatin(sourceText)
                    : Transliterator.ToRussian(sourceText);
            Clipboard.SetText(convertedText);
            return true;
        }
        catch (ExternalException exception)
        {
            error = exception.Message;
            return false;
        }
    }

    private static bool IsCyrillic(char character)
    {
        return character is >= '\u0400' and <= '\u04FF';
    }

    private static string? ReadClipboardText(uint initialSequence)
    {
        for (var attempt = 0; attempt < 30; attempt++)
        {
            var currentSequence = NativeMethods.GetClipboardSequenceNumber();
            if (Clipboard.ContainsText(TextDataFormat.UnicodeText)
                && (initialSequence == 0 || currentSequence != initialSequence))
            {
                return Clipboard.GetText(TextDataFormat.UnicodeText);
            }

            Thread.Sleep(10);
        }

        return null;
    }

    private static void RestoreClipboard(IDataObject? data)
    {
        try
        {
            if (data is null)
            {
                Clipboard.Clear();
            }
            else
            {
                Clipboard.SetDataObject(data, copy: true);
            }
        }
        catch (ExternalException)
        {
        }
    }
}
