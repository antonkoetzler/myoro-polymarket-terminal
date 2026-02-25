# Full steps: Polymarket CLOB on this computer (no copying from OpenClaw)

You only need your **private key** and your **funder (profile) address**. Then run the script here to get API key/secret/passphrase.

---

## 1. Get your private key

**If you use MetaMask with Polymarket:**  
- In MetaMask: select the account that is connected to Polymarket → three dots → Account details → Export Private Key.  
- Copy the key (with or without `0x`). This is the **owner** of your Polymarket proxy.

**If you use Email/Google (Magic) with Polymarket:**  
- Go to [reveal.magic.link/polymarket](https://reveal.magic.link/polymarket), sign in, then Reveal Private Key.  
- Copy the key and store it securely.

Put it in `.env` as:

```env
PRIVATE_KEY=your_64_char_hex_here
```

(You can use `0x` prefix or not; the script accepts both.)

---

## 2. Get your funder (proxy) address

This is the address that shows on your **Polymarket profile** and where your positions live. Orders must use this as the funder so they show under your profile.

- Go to [polymarket.com/settings](https://polymarket.com/settings).  
- Find **Wallet Address** or **Profile Address** and copy it. That is your **FUNDER_ADDRESS**.

Add it to `.env`:

```env
FUNDER_ADDRESS=0xYourProxyAddress
```

---

## 3. Run the derivation script on this computer

From the **repo root** (where `Cargo.toml` is):

```bash
pip install py-clob-client python-dotenv
python scripts/derive_polymarket_creds.py
```

The script reads `PRIVATE_KEY` from `.env` and calls Polymarket’s “create or derive” API. It will print three lines like:

```
Add these to your .env:
API_KEY=...
API_SECRET=...
API_PASSPHRASE=...
```

Copy those three into your `.env` (same file that already has `PRIVATE_KEY` and `FUNDER_ADDRESS`).

---

## 4. Set paper mode (for testing)

In `.env`:

```env
EXECUTION_MODE=paper
```

---

## 5. Final `.env` (minimal for Polymarket testing)

Your `.env` should have at least:

```env
PRIVATE_KEY=...
FUNDER_ADDRESS=0x...
API_KEY=...
API_SECRET=...
API_PASSPHRASE=...
EXECUTION_MODE=paper
```

After that, the app can load config and (once we wire the client) use the **funder** so orders show under your profile. No need to copy anything from OpenClaw; re-running the script on this machine with the same private key derives the same L2 credentials.
