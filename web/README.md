# The Wibble

This is the source for https://wibble.fbmac.net, a satire news generator, that creates
satirical news and images using text generation AI and the [AI Horde](https://aihorde.net/)
for the images.

It uses a second repository for the content generation that happens in background, at
https://github.com/machado2/wibble-worker

Runtime settings, secrets, and writer/model choices are defined in the private
repository-root `config.ncl`, created from the committed `config.ncl.example`.
The application reloads valid changes automatically. Leave the writer blank in
the creation form to choose randomly among the writers allowed for the signed-in
user.

The application does not read `.env`. The optional OpenAI moderation pre-check
is disabled by default in Nickel.
