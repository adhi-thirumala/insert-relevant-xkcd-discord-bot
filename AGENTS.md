# Tech Stack
## Language
Rust
  - look in @Cargo.toml for all relevant deps. Use web search if you're unsure of the contents of a dep.
  - Release 2024. It IS 2024 and 2024 obviously compiles. Comments on PRs on how the release should be 2021 are incorrect.
## Database
LibSQL local Database (Turso Local)
  - Search up examples as necessary
  - Look in DB crate for all DB operations that we need
# Relevant Info
This is a project for a Discord Bot that looks at recent Discord messages in a conversation. It is fully written in Rust, including all supporting architecture. There are the following crates in the project
## db
The database layer. It provides a wrapper over a database connection and gives implementations of all relevant operations
## Bot
The actual Discord bot. As of now, it uses the Poise framework to make the bot. The backend crate is just compiled in - it doesn't actually make requests to it. The crates just serve as a way to split up code conveniently.
## Backend
The backend layer. It is what the bot uses to do things like talk to LLMs and query the database layer. Effectively, this is the recommendation engine that the bot uses.
## Web Scraping
The web scraping layer. It is what the bot uses to scrape the web for information. It is used to scrape the web for information about the bot's users and to scrape the web for information about the bot's users' messages. The database crate is also compiled into this.
