# Urinal Fish

<img src="assets/logo.png" alt="Urinal Fish logo" width="220">

A small Rust Discord bot for planning nights out with friends. It creates event polls in configured Discord channels, lets people vote with buttons, remembers previous choice sets, and can post recurring event polls on a schedule.

Built with [serenity](https://github.com/serenity-rs/serenity) and SQLite.

## Features

- One-off event polls with `/event single`
- Recurring event series with `/event recurring`
- Arbitrary poll choices such as `yes,no,maybe,later`
- Previously used choice sets are remembered and suggested while typing
- One vote per user per poll; pressing another button updates their vote
- Locked to configured Discord channels through `DISCORD_CHANNEL_IDS`
- Docker Compose deployment for a Raspberry Pi
- Local SQLite database stored in the Docker volume
- Basic input hardening for poll text and choices

## Discord Setup

Create the Discord application:

1. Go to <https://discord.com/developers/applications>.
2. Click **New Application**.
3. Name it **Urinal Fish**.
4. Open **Bot** in the left sidebar.
5. If you see username, icon, and token settings, the bot user already exists. Use **Reset Token** to reveal a fresh token, copy it once, and keep it private.
6. If Discord shows an **Add Bot** button instead, click it, then copy the token.

Invite it to your server from **OAuth2** -> **URL Generator** with these scopes:

- `applications.commands`
- `bot`

Bot permissions:

- View Channel
- Send Messages
- Embed Links
- Use External Emojis is not required

Open the generated URL, choose your server, and authorize the bot.

Copy the IDs for your guild and the channels where the bot should work. Enable Developer Mode in Discord, then right-click the server/channel and use "Copy ID".

## Configure

Copy the example env file:

```sh
cp .env.example .env
```

Edit `.env`:

```env
DISCORD_TOKEN=replace-me
DISCORD_GUILD_ID=123456789012345678
DISCORD_CHANNEL_IDS=123456789012345678,234567890123456789
DATABASE_PATH=/data/bot.db
DEFAULT_TIMEZONE=Europe/Berlin
SCHEDULER_INTERVAL_SECONDS=60
```

For your setup, put the channel IDs for `a` and `urinal-test` in `DISCORD_CHANNEL_IDS`. Commands used in either channel will create polls in that same channel.

## Run

Pull the prebuilt image from GitHub Container Registry:

```sh
docker compose up -d
```

Or build a local image and run that:

```sh
docker build -t urinal-fish:local .
docker compose up -d
```

To run a local build through Compose, temporarily change `image:` in `docker-compose.yml` to `urinal-fish:local`.

Logs:

```sh
docker compose logs -f
```

Stop:

```sh
docker compose down
```

## Publishing Docker Images

The GitHub Actions workflow in `.github/workflows/docker.yml` runs tests and publishes a multi-architecture Docker image to GitHub Container Registry on pushes to `main` and version tags.

The published image name is:

```text
ghcr.io/schembriaiden/urinal-fish-bot:latest
```

For a Raspberry Pi 4, the workflow publishes `linux/arm64`. It also publishes `linux/amd64` for normal servers.

## NixOS Development

Enter the development shell:

```sh
nix develop
```

The shell provides Rust 1.96, `rustfmt`, `clippy`, `cargo-nextest`, `cargo-watch`, `sqlx-cli`, and SQLite tools.

Common commands:

```sh
cargo test
cargo clippy
cargo fmt
```

## Commands

Show a quick command guide:

```text
/help
```

Create a one-off event:

```text
/event single title: Drinks Friday when: Friday 20:00 choices: yes,no,maybe where: Berlin description: Meet outside the pub
```

Notify a user or role when the poll is posted:

```text
/event single title: Drinks Friday when: Friday 20:00 choices: yes,no,maybe where: Berlin notify: @friends
```

Create a one-off event with custom choices. The bot remembers these and suggests them the next time you type `choices`:

```text
/event single title: Food after work when: Friday 18:30 choices: pizza,sushi,no,maybe where: Berlin
```

Create a recurring event:

```text
/event recurring title: Friday drinks schedule: weekly fri 12:00 when: Friday 20:00 choices: yes,no,maybe where: Berlin
```

For recurring events, `schedule` controls when the bot posts the poll. `when` is the event time shown inside the poll.

Recurring events can also notify a user or role whenever the scheduled poll is posted:

```text
/event recurring title: Friday drinks schedule: weekly fri 12:00 when: Friday 20:00 choices: yes,no,maybe where: Berlin notify: @friends
```

Supported recurring schedules:

- `daily 19:00`
- `weekly fri 20:00`
- `friday 20:00`
- `monthly 15 19:30`

List recurring series:

```text
/series_list
```

Stop a recurring series:

```text
/series_delete id: abc12345
```

Admin-only easter egg setup:

```text
/easter_set target: @person start_time: 09:00 end_time: 22:00 message: Bring a permission slip next time.
```

Add more easter egg messages:

```text
/easter_add_message message: Your planning skills need adult supervision.
```

Check or disable it:

```text
/easter_status
/easter_disable
```

The easter egg uses the channel where `/easter_set` is run. Every day after 04:00 in `DEFAULT_TIMEZONE`, the bot rolls 1-20 once. If the roll is 11, it posts one configured message tagging the configured user at a random time between `start_time` and `end_time`.

## Single Pi Deployment

This bot is designed to run as one Docker Compose stack on one Raspberry Pi. The SQLite database lives in `./data` on the host and is mounted into the container at `/data`.

Back up the `./data` directory if you care about preserving old polls, saved choices, and recurring event settings.

## Security Notes

- SQL statements use bound parameters through SQLx instead of string-built queries.
- Commands are rejected outside `DISCORD_CHANNEL_IDS`.
- Poll titles, descriptions, "when" text, and choices have length and character validation.
- User-provided `@` mentions are neutralized before the bot reposts text into embeds/buttons.
- Easter egg setup commands require Discord administrator permission.
- Easter egg messages are stored in SQLite and mention-neutralized.
- The bot needs only Discord bot and slash-command permissions for the configured channels.

## Notes

Slash commands are registered as guild commands when the bot starts, so they should appear quickly. If you change command definitions, restart the container.
