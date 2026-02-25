# Getting started

1. **Credentials**
   - Add `.env` with `PRIVATE_KEY` and optionally `FUNDER_ADDRESS`. Run the **Derive Polymarket creds** task, then paste `API_KEY`, `API_SECRET`, `API_PASSPHRASE` into `.env`.

2. **Live data**
   - **Crypto**: BTC/USDT (Binance) + active events (Gamma API). Refreshes every 8s.
   - **Sports**: Premier League fixtures from FBRef. **Weather**: 7-day forecast (Open-Meteo, NYC). **Logs**: copy trades and errors.

3. **Copy trading**
   - Edit `copy_traders.txt` in the project root: one Polymarket profile address (0x…) per line. Lines starting with `#` are ignored. Or set `COPY_TRADERS_FILE` to another path.
   - In the **Copy** tab: **a** = add address, **d** = remove selected, **↑↓** = select. The file is reloaded when changed on disk. Trades from listed profiles appear in the tab and in **Logs**.
