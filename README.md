# OpenSwitch

OpenSwitch — open-source Windows-утилита для переключения раскладки и преобразования набранного текста.

## Возможности

- Tray-приложение для Windows.
- `Scroll Lock` — преобразовать слово под курсором.
- `Shift + Scroll Lock` — преобразовать выделенный текст.
- Автоматическое переключение по правилам и исключениям программ.
- Автозамена, история буфера, дневник и звуковые события.
- Настройки интерфейса: English, Russian, Ukrainian.
- Тёмная тема с переключателем в общих настройках.

## Сборка

Требуется .NET 8 SDK:

```powershell
dotnet restore
dotnet build .\OpenSwitch.sln --configuration Release
dotnet test .\OpenSwitch.sln --configuration Release
```

Готовый файл: `src\OpenSwitch\bin\Release\net8.0-windows\OpenSwitch.exe`.

## Настройки

Настройки сохраняются в `%APPDATA%\OpenSwitch\settings.json`.

Если одновременно запущен Punto Switcher, некоторые hotkeys могут быть заняты оригиналом. Откройте `Настройки → Горячие клавиши` и назначьте свободные сочетания.

## Иконка

При наличии `src\OpenSwitch\Assets\openswitch.ico` или `openswitch.png` приложение использует этот файл; иначе применяется встроенный fallback-значок.
