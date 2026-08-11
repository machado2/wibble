# The Wibble

This is the source for https://wibble.fbmac.net, a satire news generator, that creates
satirical news and images using text generation AI and the [AI Horde](https://aihorde.net/)
for the images.

It uses a second repository for the content generation that happens in background, at
https://github.com/machado2/wibble-worker

To run it, configure `LANGUAGE_MODEL`, either `OPENROUTER_API_KEY` or
`OPENAI_API_KEY`, and a PostgreSQL database through `DATABASE_URL`. When an
OpenRouter key is available it takes precedence. The OpenAI moderation pre-check
is opt-in through `OPENAI_MODERATION_ENABLED=true` and is disabled by default.
