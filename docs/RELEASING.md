# Выпуск новой версии

## 1. Подготовка (в ветке разработки)

1. Пройти чек-лист [`docs/CHECKLIST.md`](CHECKLIST.md) на свежей сборке (`cargo build --release`).
2. Поднять версию в `Cargo.toml` (`version = "X.Y.Z"`), затем `cargo build` (обновит `Cargo.lock`).
3. В `CHANGELOG.md` переименовать `[Unreleased]` в `[X.Y.Z] — ГГГГ-ММ-ДД` и завести новый пустой
   `[Unreleased]`.
4. Проверки: `cargo fmt`, `cargo clippy --all-targets --locked -- -D warnings`,
   `cargo test --locked`, `python tools/check_i18n.py`.
5. Коммит, слияние в `main` (через pull request — CI проверит, в том числе соберёт установщик).

## 2. Тег — дальше всё делает CI

```powershell
git tag vX.Y.Z
git push origin vX.Y.Z
```

Задание `release` в `.github/workflows/ci.yml` собирает из этого тега и кладёт в GitHub Release:

| Файл | Что это |
|---|---|
| `openswitch.exe` | сама программа |
| `OpenSwitch-vX.Y.Z-windows-x64.zip` | portable-архив (exe, LICENSE, README) |
| `OpenSwitch-X.Y.Z-setup.exe` | установщик (Inno Setup, `installer/openswitch.iss`) |
| `SHA256SUMS.txt` | контрольные суммы трёх файлов выше |
| `winget-manifests.zip` | манифесты для winget (см. ниже) |
| `openswitch.json` | манифест для Scoop (см. ниже) |

Для exe, архива и установщика CI делает аттестацию происхождения сборки (build provenance):
проверить можно командой `gh attestation verify <файл> --repo DezFix/OpenSwitch`.

## 3. winget (вручную, один раз на версию)

Манифесты готовы в `winget-manifests.zip` (идентификатор пакета `DezFix.OpenSwitch`).

Самый простой способ — [wingetcreate](https://github.com/microsoft/winget-create):

```powershell
winget install wingetcreate
wingetcreate submit --token <GitHub-токен> <распакованная папка>\winget\manifests\d\DezFix\OpenSwitch\X.Y.Z
```

Или вручную: форк `microsoft/winget-pkgs`, положить папку `manifests/d/DezFix/OpenSwitch/X.Y.Z`
из архива, pull request. Перед отправкой можно проверить: `winget validate <папка>`.
Первая версия проходит ручную проверку модераторов (обычно несколько дней).

## 4. Scoop

- Сразу, без каталога: `scoop install https://github.com/DezFix/OpenSwitch/releases/download/vX.Y.Z/openswitch.json`.
- В каталог: pull request с `openswitch.json` в бакет [ScoopInstaller/Extras](https://github.com/ScoopInstaller/Extras)
  (папка `bucket/`). Манифест содержит `checkver` и `autoupdate`, поэтому дальше бакет
  обновляется сам.

## 5. Подпись кода

Пока программа не подписана, SmartScreen при первом запуске показывает «Windows защитила ваш
компьютер» (нужно нажать «Подробнее → Выполнить в любом случае»), а антивирусы чаще
подозревают программу с глобальным хуком клавиатуры. План — бесплатная подпись для open source
от [SignPath Foundation](https://signpath.org/): подать заявку от имени владельца репозитория;
после одобрения в CI добавляется шаг подписи exe и установщика (см. `ROADMAP.md`, этап 0.8).
