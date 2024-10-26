# Minions

A framework for building performant and secure PWAs in (mostly) full-stack Rust. 

We aim to provide simple building blocks for apps that are fast, secure, and easy to build.
Open source widget libraries are integrated to Nostr data feeds to provide interactivity and 
real-time data updates.

## Build and run

The app uses Yew to build a WASM client side application. Styling is handled using TailwindCSS.


To build and run the project, you should have installed:

- [Rust](https://www.rust-lang.org/)
- [TailwindCSS](https://tailwindcss.com/)
- [Trunk](https://trunkrs.dev/)

To build a new application, clone the repository and run the following command:

```bash
trunk serve
```

This will build the project and start a development server.

### "Warm" Reloading

The build script will recognize any HTML content inside Yew's functional components and reload new changes on save.
Due to Rust's compile times, this will not be instant.

### Contributing

Bounties to help drive open contribution forward will be posted as issues on the repository.
All rewards are paid in Bitcoin.

