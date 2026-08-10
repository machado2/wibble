# The Wibble

This is the source for https://wibble.fbmac.net, a satire news generator, that creates
satirical news and images using text generation AI and the [AI Horde](https://aihorde.net/)
for the images.

It uses a second repository for the content generation that happens in background, at
https://github.com/machado2/wibble-worker

To run it, you need an OPENAI API Key, an AI Horde key (this one is free),
and a PostgreSQL database, configured through `DATABASE_URL` in the environment.
