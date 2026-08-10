# The Wibble

This is the source for https://wibble.news, a satire news generator, that creates
satirical news and images using text generation AI and the [AI Horde](https://aihorde.net/)
for the images.

It uses a second repository for the content generation that happens in background, at
https://github.com/machado2/wibble-worker

To run it, you need an OPENAI API Key, an AI Horde key (this one is free),
and a mysql database, that you need to configure in the .env file.
