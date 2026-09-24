using System.Media;

namespace OpenSwitch;

public sealed class SoundService
{
    private AppSettings settings;

    public SoundService(AppSettings settings)
    {
        this.settings = settings;
    }

    public void ApplySettings(AppSettings updatedSettings)
    {
        settings = updatedSettings;
    }

    public void Play(string eventName)
    {
        var sound = settings.SoundEvents.FirstOrDefault(item =>
            item.Enabled && string.Equals(item.Event, eventName, StringComparison.OrdinalIgnoreCase));
        if (sound is null)
        {
            return;
        }

        if (!string.IsNullOrWhiteSpace(sound.FilePath) && File.Exists(sound.FilePath))
        {
            using var player = new SoundPlayer(sound.FilePath);
            player.Play();
            return;
        }

        if (sound.UseSystemSound)
        {
            SystemSounds.Asterisk.Play();
        }
    }
}
