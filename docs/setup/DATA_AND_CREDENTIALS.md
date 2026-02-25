# Data sources and credentials

## 1. Polymarket: profile vs wallet, proxy (funder) address — trades under your profile

**Why trades didn’t show under your profile (OpenClaw):**  
On Polymarket, your **profile** is tied to a **proxy wallet** (a Gnosis Safe on Polygon), not your MetaMask EOA. Your MetaMask is the **owner** of that Safe. The CLOB attributes orders to a **funder address**. If the client sends the EOA (MetaMask) as the funder, orders don’t show under your Polymarket profile; they must use the **proxy (Safe) address** as the funder.

**What to do so trades show under your profile:**  
- Use your **Polymarket public address** (the proxy/Safe address) as the **funder** when creating the CLOB client. Sign with your MetaMask key (owner), but set the funder/poly address to the proxy.  
- **Get your proxy address:** In Polymarket, your profile URL or settings may show it; or on PolygonScan, look up your MetaMask address and find the Safe of which it is owner (created by the Gnosis Safe factory `0xaacfeea03eb1561c4e67d661e40682bd20e3541b`). That Safe address is your Polymarket “public address”.  
- **In this app:** Set `FUNDER_ADDRESS` (or `POLY_ADDRESS`) in `.env` to that proxy address. When we wire the Polymarket client, we will pass this as the funder so all orders attribute to your profile.  
- **OpenClaw:** Reuse OpenClaw’s `API_KEY`, `API_SECRET`, `API_PASSPHRASE` **only if** OpenClaw is already configured with the same funder (proxy) address. If OpenClaw was using the EOA and that’s why trades didn’t show, fix the funder in OpenClaw or here; here we’ll use `FUNDER_ADDRESS` from env so orders show under your profile.

---

## 2. Sports: 10 req/min vs scraping — plan

**10 req/min (football-data.org)** is tight for many leagues/matches. **Scraping is a good free alternative** and latency is fine for sports (pattern work, not HFT).

**What we need:** Fixtures, results, standings (and optionally other books’ odds to compare to Polymarket).

**Plan:**  
- **Primary:** Scrape **one** site that has fixtures + results + standings (and optionally odds). Examples: FlashScore, SofaScore, FBRef. Avoid scattering across many sites at first.  
- **Odds:** Polymarket is the only book we need for *trading*. Scraping other sportsbooks (e.g. Oddschecker, individual books) is optional and only if we want cross-book comparison or arb; not required for “Polymarket-only” strategies.  
- **Scope:** Start soccer-only (one league or a small set). Add more sports/sites later if needed.

So: **yes, prefer scraping over football-data.org** for a free, flexible setup. Pick one site with fixtures/results/standings (and optionally odds) and scrape that.

---

## 3. NOAA: API key or not?

- **api.weather.gov** — **No API key.** Use for **current and forecast** weather (e.g. “will temp be above X°C on date D”). Enough for prediction markets.  
- **NOAA CDO** (Climate Data Online) — **Free token (API key) required.** Use only if we need **historical** climate data.  
**Summary:** For our use (forecasts for weather markets): **no key.** Use api.weather.gov only.

---

## 4. Weather: free APIs only; no key, no OWM required

- **Open-Meteo** — **No API key.** Free, 10,000 calls/day, global. Use for forecasts (temp, etc.). `https://api.open-meteo.com/v1/forecast?latitude=...&longitude=...`  
- **api.weather.gov** — **No API key.** US only, forecasts. Good for US city markets.  
- **OpenWeatherMap** — Optional; 1,000 calls/day free, needs key. Not required; Open-Meteo + weather.gov are enough.  
- **Scraping weather:** Not needed; free APIs cover forecasts.  
**Recommendation:** Use **Open-Meteo** as primary (global, no key). Add **api.weather.gov** for US if we want a second source. Skip OWM unless we hit limits. No scraping for weather.
