namespace OpenSwitch;

public static class TextFixer
{
    private static readonly Dictionary<string, string> Abbreviations = new(StringComparer.OrdinalIgnoreCase)
    {
        ["т.е."] = "то есть",
        ["т.д."] = "и так далее",
        ["и.о."] = "исполняющий обязанности",
        ["др."] = "другие"
    };

    public static string Fix(string word, AppSettings settings)
    {
        var result = word;
        if (settings.FixDuplicateLetters)
        {
            result = RemoveDuplicatePrefix(result);
        }

        if (settings.FixAbbreviations && Abbreviations.TryGetValue(result, out var replacement))
        {
            result = replacement;
        }

        return result;
    }

    private static string RemoveDuplicatePrefix(string value)
    {
        if (value.Length < 2)
        {
            return value;
        }

        var first = char.ToLowerInvariant(value[0]);
        var duplicateCount = 1;
        while (duplicateCount < value.Length
            && char.ToLowerInvariant(value[duplicateCount]) == first)
        {
            duplicateCount++;
        }

        return duplicateCount > 1 ? value[1..] : value;
    }
}
