namespace OpenSwitch;

internal static class Program
{
    [STAThread]
    private static void Main()
    {
        using var mutex = new Mutex(true, @"Local\OpenSwitch", out var createdNew);
        if (!createdNew)
        {
            return;
        }

        Application.SetUnhandledExceptionMode(UnhandledExceptionMode.CatchException);
        Application.ThreadException += (_, args) => CrashHandler.Log(args.Exception);
        AppDomain.CurrentDomain.UnhandledException += (_, args) =>
        {
            if (args.ExceptionObject is Exception exception)
            {
                CrashHandler.Log(exception);
            }
        };

        try
        {
            ApplicationConfiguration.Initialize();
            using var context = new TrayApplicationContext();
            Application.Run(context);
        }
        catch (Exception exception)
        {
            CrashHandler.Log(exception);
            MessageBox.Show(
                $"OpenSwitch stopped unexpectedly.{Environment.NewLine}{Environment.NewLine}{exception.Message}",
                "OpenSwitch",
                MessageBoxButtons.OK,
                MessageBoxIcon.Error);
        }
    }
}
