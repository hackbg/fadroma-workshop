# Fadroma Workshop

Requirements: Git, Node >=16, Cargo, Docker.

1. Run the project creation wizard: `npx @hackbg/fadroma@1.4.12 create`
2. Enter your project directory: `cd projectname`
3. Install Node dependencies: `npm i`
4. Install Rust dependencies, generating initial lockfile: `cargo update`
5. Compile the contracts: `npm run build`
6. Deploy the contracts to a local devnet: `npm run devnet deploy`

Once you've confirmed that the above works,
help yourself to the [contracts walkthrough](./WALKTHROUGH.md)
and the [deployment guide](./FACTORY.md). Happy hacking!

---

Powered by [Fadroma](https://fadroma.tech) by [Hack.bg](https://hack.bg) under [AGPL3](https://www.gnu.org/licenses/agpl-3.0.en.html).
