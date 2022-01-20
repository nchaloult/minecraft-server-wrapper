# mc-server-wrapper

A small utility that wraps a Java Minecraft server process and exposes HTTP APIs that let you interact with that server.

Built to wrap a vanilla Minecraft server — not servers with [Fabric](https://fabricmc.net) or [Forge](https://mcforge.readthedocs.io/en/1.18.x/), for instance.

## Installation

1. Get a `mc-server-wrapper` binary
   - Either by downloading one of the prepared ones in this repo's releases **(TODO)**
   - Or by building this project from source
1. Follow the steps that you normally would to stand up a vanilla Minecraft server without this wrapper
   - If you've never done this before, there are plenty of resources online about this. [Here](https://help.minecraft.net/hc/en-us/articles/360058525452-How-to-Setup-a-Minecraft-Java-Edition-Server) are [some](https://blogs.oracle.com/developers/post/how-to-set-up-and-run-a-really-powerful-free-minecraft-server-in-the-cloud) to [get](https://www.cloudskillsboost.google/focuses/1852?parent=catalog) you [started](https://dev.to/julbrs/how-to-run-a-minecraft-server-on-aws-for-less-than-3-us-a-month-409p)
1. Move the `mc-server-wrapper` binary into the same directory as the `server.jar` file that Mojang provides

## Usage

Each time you want to launch the server, run the wrapper instead of running the server `.jar` that Mojang provides.

```bash
# Instead of something like:
# $ java [...] -jar server.jar [...]
# Run this instead:
./mc-server-wrapper
```

### Command-Line Functionality

Normally, the primary way to interact with a vanilla Minecraft server is by entering commands into an interactive process that the `server.jar` spawns. `mc-server-wrapper` doesn't compromise this functionality — it captures user input and passes it to that process's `stdin`. If you'd like, you can interact with the Minecraft server as if the wrapper weren't there.

### HTTP APIs

- `GET /list-players`: Get a list of the usernames of all players who are currently logged in
- `GET /stop`: Gracefully shut down the Minecraft server, and stop listening for more incoming HTTP requests
