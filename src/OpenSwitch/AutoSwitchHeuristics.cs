namespace OpenSwitch;

public static class AutoSwitchHeuristics
{
    private static readonly HashSet<string> RussianWords = new(StringComparer.OrdinalIgnoreCase)
    {
        "привет",
        "здравствуйте",
        "спасибо",
        "пожалуйста",
        "извините",
        "добрый",
        "день",
        "вечер",
        "работа",
        "текст",
        "слово",
        "буфер",
        "написать",
        "исправление",
        "настройки",
        "программа"
    };

    private static readonly HashSet<string> EnglishWords = new(StringComparer.OrdinalIgnoreCase)
    {
        "hello",
        "hi",
        "thanks",
        "please",
        "sorry",
        "good",
        "day",
        "night",
        "work",
        "text",
        "word",
        "clipboard",
        "settings",
        "program",
        "ghbdtn",
        "rjvf"
    };

    public static bool ShouldSwitch(string word)
    {
        if (string.IsNullOrWhiteSpace(word))
        {
            return false;
        }

        var hasCyrillic = word.Any(character => character is >= '\u0400' and <= '\u04FF');
        var candidate = LayoutConverter.Convert(word).Text;
        return hasCyrillic
            ? EnglishWords.Contains(candidate)
            : RussianWords.Contains(candidate);
    }
}
