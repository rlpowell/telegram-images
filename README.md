I got tired of having to manually save images and videos out of Telegram and into my normal photo workflow, so I wrote a Telegram API program to do it for me.

That's perfectly normal and reasonable, right?  See also https://github.com/rlpowell/smsbar-images

It's all podman/docker based; my lunvarvim_rust image is used as a basis ( see https://github.com/rlpowell/lunarvim_rust ), but honestly any image with a working Rust should do fine.

Note that this uses the Telegram API, which is *not* the Telegram Bot API (ugh).  It acts as your own user on Telegram, not as a bot.

tdlib, the underlying Telegram library we're using, has a concept of an encrypted database that lives locally and stores details about your interactions with Telegram.  This program makes a directory named telegram_database/ for that in the current dir.  You want to preserver that, or you'll need to re-auth on every run.

This database is encrypted with a key (i.e. a string, more-or-less a password) that you provide.  This code just uses the empty string, because encrypting that data is not interesting to me at all for my use case; if you want to do something better, change the return value of handle_encryption_key.

To run, copy Settings.toml.example to Settings.toml and fill in your api_id and api_hash from https://my.telegram.org/apps

When you first run this program, it'll ask you for your Telegram phone number, but it shouldn't ask after that as long you preserve the telegram_database/ directory.  Once it's asked for your phone number, you'll get an auth request via Telegram, which you need to copy.

If something goes wrong with the auth workflow and you have to try again, delete the telegram_database/ directory.
