using System.Globalization;
using System.Reflection;
using System.Text.Json;

namespace OpenSwitch;

public enum AppLanguage
{
    Auto,
    English,
    Russian,
    Ukrainian
}

public sealed record LanguageOption(AppLanguage Value, string Code, string DisplayName);

public sealed class Localizer
{
    private static readonly Lazy<IReadOnlyDictionary<string, IReadOnlyDictionary<string, string>>> Catalog =
        new(LoadCatalog);

    private AppLanguage language;

    public Localizer(AppLanguage language)
    {
        this.language = language;
    }

    public event EventHandler? LanguageChanged;

    public AppLanguage Language => language;

    public string this[string key] => Get(key);

    public IReadOnlyList<LanguageOption> AvailableLanguages { get; } =
    [
        new LanguageOption(AppLanguage.Auto, "auto", "Auto"),
        new LanguageOption(AppLanguage.English, "en", "English"),
        new LanguageOption(AppLanguage.Russian, "ru", "Russian"),
        new LanguageOption(AppLanguage.Ukrainian, "uk", "Ukrainian")
    ];

    public void SetLanguage(AppLanguage value)
    {
        if (language == value)
        {
            return;
        }

        language = value;
        LanguageChanged?.Invoke(this, EventArgs.Empty);
    }

    public string Get(string key)
    {
        var languageCode = ResolveLanguageCode(language);
        var catalog = Catalog.Value;
        if (catalog.TryGetValue(languageCode, out var localized)
            && localized.TryGetValue(key, out var localizedValue))
        {
            return localizedValue;
        }

        return catalog.TryGetValue("en", out var english)
            && english.TryGetValue(key, out var englishValue)
                ? englishValue
                : key;
    }

    public static AppLanguage ParseLanguage(string? value)
    {
        return value?.ToLowerInvariant() switch
        {
            "en" => AppLanguage.English,
            "ru" => AppLanguage.Russian,
            "uk" => AppLanguage.Ukrainian,
            _ => AppLanguage.Auto
        };
    }

    public static string ToStorageValue(AppLanguage language)
    {
        return language switch
        {
            AppLanguage.English => "en",
            AppLanguage.Russian => "ru",
            AppLanguage.Ukrainian => "uk",
            _ => "auto"
        };
    }

    private static string ResolveLanguageCode(AppLanguage language)
    {
        if (language != AppLanguage.Auto)
        {
            return language switch
            {
                AppLanguage.English => "en",
                AppLanguage.Russian => "ru",
                AppLanguage.Ukrainian => "uk",
                _ => "en"
            };
        }

        var cultureCode = CultureInfo.CurrentUICulture.TwoLetterISOLanguageName.ToLowerInvariant();
        return cultureCode is "ru" or "uk" ? cultureCode : "en";
    }

    private static IReadOnlyDictionary<string, IReadOnlyDictionary<string, string>> LoadCatalog()
    {
        var assembly = Assembly.GetExecutingAssembly();
        var resourceName = assembly.GetManifestResourceNames()
            .FirstOrDefault(name => name.EndsWith("locales.json", StringComparison.OrdinalIgnoreCase));
        if (resourceName is null)
        {
            return new Dictionary<string, IReadOnlyDictionary<string, string>>();
        }

        using var stream = assembly.GetManifestResourceStream(resourceName);
        if (stream is null)
        {
            return new Dictionary<string, IReadOnlyDictionary<string, string>>();
        }

        using var document = JsonDocument.Parse(stream);
        var result = new Dictionary<string, IReadOnlyDictionary<string, string>>();
        foreach (var languageProperty in document.RootElement.EnumerateObject())
        {
            var strings = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            foreach (var stringProperty in languageProperty.Value.EnumerateObject())
            {
                strings[stringProperty.Name] = stringProperty.Value.GetString() ?? string.Empty;
            }

            result[languageProperty.Name] = strings;
        }

        return result;
    }
}
