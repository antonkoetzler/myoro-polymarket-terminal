# What You Need to Provide

## Polymarket (required for trading)

- **FUNDER_ADDRESS** — Your Polymarket **proxy (Safe) address** so orders show under your profile. Not your MetaMask address. Get it from your Polymarket profile or PolygonScan (Safe owned by your EOA). See [docs/DATA_AND_CREDENTIALS.md](docs/DATA_AND_CREDENTIALS.md) for why.
- **PRIVATE_KEY** — Your Ethereum wallet (MetaMask) private key; the owner of the proxy. The SDK derives CLOB API credentials from it.
  - **How:** Export from MetaMask. Use a dedicated trading wallet.
- **Optional:** **API_KEY**, **API_SECRET**, **API_PASSPHRASE** — Pre-derived CLOB credentials (e.g. from OpenClaw). If set, the app uses these. Must be for the same funder (proxy) so trades show on your profile.

## Binance (optional; for crypto strategies)

- **BINANCE_API_KEY** — Only needed if we use authenticated endpoints. Public WebSocket (e.g. kline, bookTicker) often works without a key.
  - **How:** [Binance](https://www.binance.com/) → Profile → API Management → Create API. Restrict to “Enable Reading” if you only need data.

---

Put credentials in `.env` (see `.env.example`). Never commit `.env`.  
App settings (paper bankroll, execution mode, copy traders, poll interval) live in `config.json` (see `config.json.example`).
