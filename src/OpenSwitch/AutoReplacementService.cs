namespace OpenSwitch;

public sealed class AutoReplacementService
{
    private AppSettings settings;

    public AutoReplacementService(AppSettings settings)
    {
        this.settings = settings;
    }

    public void ApplySettings(AppSettings updatedSettings)
    {
        settings = updatedSettings;
    }

    public bool TryGetReplacement(string word, string triggerKey, out string replacement)
    {
        replacement = string.Empty;
        foreach (var item in settings.Replacements)
        {
            if (!item.Enabled
                || !string.Equals(item.TriggerKey, triggerKey, StringComparison.OrdinalIgnoreCase)
                || !string.Equals(item.Find, word, StringComparison.OrdinalIgnoreCase))
            {
                continue;
            }

            replacement = item.ReplaceWith;
            return true;
        }

        return false;
    }
}
