using System.Text;

namespace OpenSwitch;

public enum LayoutDirection
{
    LatinToCyrillic,
    CyrillicToLatin
}

public readonly record struct ConversionResult(
    string Text,
    LayoutDirection Direction,
    int ReplacedCharacters);

public static class LayoutConverter
{
    private static readonly Dictionary<char, char> LatinToCyrillicMap = CreateMap(
        "qwertyuiop[]asdfghjkl;'zxcvbnm,./",
        "йцукенгшщзхъфывапролджэячсмитьбю.");

    private static readonly Dictionary<char, char> CyrillicToLatinMap = CreateReverseMap(LatinToCyrillicMap);

    public static ConversionResult Convert(string? text)
    {
        if (string.IsNullOrEmpty(text))
        {
            return new ConversionResult(string.Empty, LayoutDirection.LatinToCyrillic, 0);
        }

        var direction = text.Any(IsCyrillic)
            ? LayoutDirection.CyrillicToLatin
            : LayoutDirection.LatinToCyrillic;
        var map = direction == LayoutDirection.CyrillicToLatin
            ? CyrillicToLatinMap
            : LatinToCyrillicMap;
        var builder = new StringBuilder(text.Length);
        var replacedCharacters = 0;

        foreach (var character in text)
        {
            var key = char.ToLowerInvariant(character);
            if (!map.TryGetValue(key, out var replacement))
            {
                builder.Append(character);
                continue;
            }

            builder.Append(char.IsUpper(character) ? char.ToUpperInvariant(replacement) : replacement);
            replacedCharacters++;
        }

        return new ConversionResult(builder.ToString(), direction, replacedCharacters);
    }

    private static bool IsCyrillic(char character)
    {
        return character is >= '\u0400' and <= '\u04FF';
    }

    private static Dictionary<char, char> CreateMap(string source, string target)
    {
        if (source.Length != target.Length)
        {
            throw new ArgumentException("Keyboard layout maps must have equal lengths.");
        }

        var map = new Dictionary<char, char>(source.Length);
        for (var index = 0; index < source.Length; index++)
        {
            map.Add(source[index], target[index]);
        }

        return map;
    }

    private static Dictionary<char, char> CreateReverseMap(Dictionary<char, char> source)
    {
        return source.ToDictionary(pair => pair.Value, pair => pair.Key);
    }
}
