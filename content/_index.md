+++
title = "Deplorable Repo Builder"
date = "2014-04-09"
layout = "_default"
+++

_Deplorable_ is a simple & small daemon that deploys your code.

### What is it?

_Deplorable_ is a background process and HTTP server. It accepts web hooks from
your favorite collaborative version control service[^1] whenever you push to a
particular branch, fetches the most recent code from your repository, and
"builds" it using the [Nix](https://nixos.org/) expression in your repository
(in `default.nix`).

### When can I use it?

Now!

### Where can I use it?

It has been tested to work running as a systemd service on NixOS, but it should
work anywhere a compiled Rust program can run, listen on a TCP socket, and
invoke the Nix package manager. This _should_ include any Linux distribution, as
well as OS X, and potentially Windows and BSD if you're willing to put in some
elbow grease.

You don't have to use systemd to run it. You can use SysVinit scripts, a
tmux/screen session, launchd, upstart... you do you. Systemd is nice because
deplorable will likely end up running with a fairly low PID number, and that's
apparently important for some reason.

### Why would I use it?

Suppose you're at a large institution that has a policy that makes it hard to
point subdomains at Netlify or GitHub pages, but easy to get public IPv4
addresses and host web sites on your workstation, a server in the basement, or
an on-premise virtual machine. Deplorable can help you get the experience of
using these services without the headache of arguing with anyone about whether
hosting a website on GitHub does or does not violate internal policies or
whether internal policies should change.

Alternatively, suppose you want 

  * Automatically deploying static web sites using static site generators such as Jekyll or Hugo on your own server.

  * Automatically deploying static web sites using your own framework, or some
    other framework not supported by major static site hosts.

  * That's pretty much it? Any other reason you might have to keep the most
    recent version of your code compiled on some computer somewhere.

### How do I use it?

Great question!

You'll need five things:

1. (Optional, but actually required for now) a working Rust compiler and the
   Rust package manager Cargo. You only need this if you want to compile
   deplorable from source. Howoever, at present, you almost certainly need to
   compile it from source. If you're using Nix to compile deplorable, this will
   already be taken care of for you.

2. Some basic dependencies including libcurl and openssl. If you're using Nix
   to build deplorable (see below), this is already taken care of for you. See
   how nice a meta!

3. One or more GitHub repositories you want to deploy automatically.

4. A computer connected to the Internet that can accept web hooks (a public IP
   address is ideal, but tools such as ngrok and smee can help if you're behind
   a NAT or are otherwise unable to listen directly for TCP connection on the
   public Internet).

4. The Nix package manager (instructions
   [here](https://nixos.org/nix/manual/#ch-installing-binary)). Note that you
   do not need eleveated privileges to install or use the Nix package manager.

#### Build

First, get the source code. There are various ways you can do this, but let's use Git 'cause why the heck not:

```bash
$ git clone git://github.com/alevy/deplorable
$ cd deplorable
```

Next, you have a choice. You can build deplorable with Rust's `cargo` or you
can build deplorable with Nix (which, in turn, will invoke Rust).

```bash
$ cargo build --release
# outputs to $PWD/target/release/deplorable
```

or

```bash
$ nix build
# outputs to $PWD/result/bin/deplorable
```

#### Configure

Deplorable uses a YAML (meaning it can also be JSON) configuration file to determine which repositories it
expects to receive web hooks from and which credentials (if any) it should use. The configuration file has a top-level "repos" key that resolves to a list of repository objects. The key for each repository will correspond to it's webhook path.

Each repository object MUST include at least the following entries:

  * `repo`: the full GitHub repo name, i.e. `{owner}/{repo}`.
  * `reference`: the full reference to watch and deploy from, including the `refs/heads/` prefix. Typically, this might be something like `refs/heads/master` or `refs/heads/staging`.
  * `out`: a relative or absolute path where the repo should be compiled to. Whichever user is running `deplorable` must be able to write to this location.

Optionally, a repository object may also contain:

  * `secret`: the shared GitHub webhook secret used to authenticate the GitHub webhook. It is not necessary to configure a shared secret, but recommended to avoid spurious builds.
  * `token`: an GitHub OAuth token that permits access to the repository if it is private.

The following example shows a configuration for two totally non-existant repositories, one public, one private.

```yaml
repos:
  # Site 1 is in a public repo
  site1:
    repo: myuser/mysite1
    reference: refs/heads/master
    out: /var/www/site1
    secret: mysecretkey1
  # Site 2 is in a private repo, so it needs a token to access
  site2:
    repo: myuser/mysite2
    reference: refs/heads/beta
    out: /var/www/site2
    secret: mysecretkey2
    token: AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
```

#### Run

Run the damn thing!

```bash
$ target/release/deplorable -c YOUR_CONFIG_FILE.yaml -l 0.0.0.0:1337
Listening for web hooks on 0.0.0.0:1337
```

If you'd like to expose deplorable over TLS and/or on ports 80 or 443, you might consider reverse proxying, e.g., through NGINX or Apache.

#### Setup Web Hooks

Follow GitHub's [guide](https://docs.github.com/en/developers/webhooks-and-events/creating-webhooks) to setting up a webhook for your repository.

For each repository, the "Payload URL" is:

```
http://YOUR_IP_OR_DOMAIN_NAME:1337/REPO_KEY_FROM_CONFIGURATION
```

Select `application/json` for the content type and ensure the secret matches the one in your configuration file. Finally, you only need to activate Push events, which is the default. Other events will be ignored anyway.

## FAQs

1. Why is it called _deplorable_?

It helps you "deploy" things, and I needed to name the repository something.
Please don't read into it.

[^1]: as long as your favorite collaborative version control service is GitHub.
