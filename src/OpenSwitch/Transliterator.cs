using System.Text;

namespace OpenSwitch;

public static class Transliterator
{
    private static readonly Dictionary<char, string> RussianToLatin = new()
    {
        ['а'] = "a",
        ['б'] = "b",
        ['в'] = "v",
        ['г'] = "g",
        ['д'] = "d",
        ['е'] = "e",
        ['ё'] = "yo",
        ['ж'] = "zh",
        ['з'] = "z",
        ['и'] = "i",
        ['й'] = "y",
        ['к'] = "k",
        ['л'] = "l",
        ['м'] = "m",
        ['н'] = "n",
        ['о'] = "o",
        ['п'] = "p",
        ['р'] = "r",
        ['с'] = "s",
        ['т'] = "t",
        ['у'] = "u",
        ['ф'] = "f",
        ['х'] = "h",
        ['ц'] = "ts",
        ['ч'] = "ch",
        ['ш'] = "sh",
        ['щ'] = "shch",
        ['ъ'] = string.Empty,
        ['ы'] = "y",
        ['ь'] = string.Empty,
        ['э'] = "e",
        ['ю'] = "yu",
        ['я'] = "ya"
    };

    private static readonly Dictionary<string, string> LatinToRussian = new(StringComparer.OrdinalIgnoreCase)
    {
        ["shch"] = "щ",
        ["sch"] = "щ",
        ["yo"] = "ё",
        ["zh"] = "ж",
        ["ts"] = "ц",
        ["ch"] = "ч",
        ["sh"] = "ш",
        ["yu"] = "ю",
        ["ya"] = "я",
        ["a"] = "а",
        ["b"] = "б",
        ["v"] = "в",
        ["g"] = "г",
        ["d"] = "д",
        ["e"] = "е",
        ["z"] = "з",
        ["i"] = "и",
        ["j"] = "й",
        ["k"] = "к",
        ["l"] = "л",
        ["m"] = "м",
        ["n"] = "н",
        ["o"] = "о",
        ["p"] = "п",
        ["r"] = "р",
        ["s"] = "с",
        ["t"] = "т",
        ["u"] = "у",
        ["f"] = "ф",
        ["h"] = "х",
        ["c"] = "ц",
        ["w"] = "ш",
        ["q"] = "к",
        ["y"] = "ы",
        ["x"] = "х",
        ["ь"] = "ь"
    };

    private static readonly string[] LatinKeys = LatinToRussian.Keys
        .OrderByDescending(key => key.Length)
        .ToArray();

    public static string ToLatin(string? text)
    {
        if (string.IsNullOrEmpty(text))
        {
            return string.Empty;
        }

        var builder = new StringBuilder(text.Length);
        foreach (var character in text)
        {
            if (RussianToLatin.TryGetValue(char.ToLowerInvariant(character), out var replacement))
            {
                builder.Append(char.IsUpper(character) ? replacement.ToUpperInvariant() : replacement);
            }
            else
            {
                builder.Append(character);
            }
        }

        return builder.ToString();
    }

    public static string ToRussian(string? text)
    {
        if (string.IsNullOrEmpty(text))
        {
            return string.Empty;
        }

        var builder = new StringBuilder(text.Length);
        for (var index = 0; index < text.Length;)
        {
            if (!char.IsLetter(text[index]))
            {
                builder.Append(text[index++]);
                continue;
            }

            var key = LatinKeys.FirstOrDefault(candidate =>
                index + candidate.Length <= text.Length
                && text.AsSpan(index, candidate.Length).Equals(candidate, StringComparison.OrdinalIgnoreCase));
            if (key is null)
            {
                builder.Append(text[index++]);
                continue;
            }

            var replacement = LatinToRussian[key];
            builder.Append(char.IsUpper(text[index]) ? char.ToUpperInvariant(replacement[0]) : replacement[0]);
            index += key.Length;
        }

        return builder.ToString();
    }
}
