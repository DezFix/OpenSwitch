using OpenSwitch;

namespace OpenSwitch.Tests;

public sealed class LayoutConverterTests
{
    [Fact]
    public void ConvertsLatinTextTypedAsRussianLayout()
    {
        var result = LayoutConverter.Convert("ghbdtn");

        Assert.Equal("привет", result.Text);
        Assert.Equal(LayoutDirection.LatinToCyrillic, result.Direction);
        Assert.Equal(6, result.ReplacedCharacters);
    }

    [Fact]
    public void ConvertsCyrillicTextTypedAsLatinLayout()
    {
        var result = LayoutConverter.Convert("привет");

        Assert.Equal("ghbdtn", result.Text);
        Assert.Equal(LayoutDirection.CyrillicToLatin, result.Direction);
        Assert.Equal(6, result.ReplacedCharacters);
    }

    [Fact]
    public void PreservesCaseWhitespaceAndDigits()
    {
        var result = LayoutConverter.Convert("Ghbdtn 123!");

        Assert.Equal("Привет 123!", result.Text);
    }

    [Fact]
    public void ReturnsEmptyResultForEmptyText()
    {
        var result = LayoutConverter.Convert(string.Empty);

        Assert.Equal(string.Empty, result.Text);
        Assert.Equal(0, result.ReplacedCharacters);
    }
}

public sealed class TransliteratorTests
{
    [Fact]
    public void TransliteratesRussianTextToLatin()
    {
        Assert.Equal("privet", Transliterator.ToLatin("привет"));
    }

    [Fact]
    public void TransliteratesLatinTextToRussian()
    {
        Assert.Equal("привет", Transliterator.ToRussian("privet"));
    }
}

public sealed class AutoSwitchHeuristicsTests
{
    [Fact]
    public void RecognizesCommonRussianWordTypedInLatinLayout()
    {
        Assert.True(AutoSwitchHeuristics.ShouldSwitch("ghbdtn"));
    }

    [Fact]
    public void RecognizesCommonRussianWordTypedInRussianLayout()
    {
        Assert.True(AutoSwitchHeuristics.ShouldSwitch("привет"));
    }

    [Fact]
    public void DoesNotSwitchUnknownEnglishWord()
    {
        Assert.False(AutoSwitchHeuristics.ShouldSwitch("hello"));
    }
}

public sealed class LocalizationTests
{
    [Fact]
    public void LoadsRussianTranslation()
    {
        var localizer = new Localizer(AppLanguage.Russian);

        Assert.Equal("Настройки", localizer.Get("Settings"));
        Assert.Equal("Горячие клавиши", localizer.Get("Hotkeys"));
    }

    [Fact]
    public void LoadsUkrainianTranslation()
    {
        var localizer = new Localizer(AppLanguage.Ukrainian);

        Assert.Equal("Налаштування", localizer.Get("Settings"));
        Assert.Equal("Гарячі клавіші", localizer.Get("Hotkeys"));
    }
}

public sealed class TextFixerTests
{
    [Fact]
    public void RemovesDuplicatePrefixWhenEnabled()
    {
        var settings = new AppSettings { FixDuplicateLetters = true };

        Assert.Equal("привет", TextFixer.Fix("ппривет", settings));
    }

    [Fact]
    public void LeavesTextUnchangedWhenFixIsDisabled()
    {
        var settings = new AppSettings();

        Assert.Equal("ппривет", TextFixer.Fix("ппривет", settings));
    }
}

public sealed class AutoReplacementServiceTests
{
    [Fact]
    public void FindsConfiguredReplacement()
    {
        var settings = new AppSettings
        {
            Replacements =
            [
                new AutoReplacement
                {
                    Find = "ghbdtn",
                    ReplaceWith = "привет",
                    TriggerKey = "Enter"
                }
            ]
        };
        var service = new AutoReplacementService(settings);

        var found = service.TryGetReplacement("ghbdtn", "Enter", out var replacement);

        Assert.True(found);
        Assert.Equal("привет", replacement);
    }
}
