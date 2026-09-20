#!/usr/bin/env python3
"""
Check DigiKey stock/pricing for every MPN in a BOM CSV via the Product Information API v4.

Setup:
  1. developer.digikey.com -> create an Organization + Production App,
     enable "ProductInformation V4". Copy Client ID / Secret.
  2. export DIGIKEY_CLIENT_ID=...  DIGIKEY_CLIENT_SECRET=...
      (CLIENT_ID and CLIENT_SECRET are also accepted from .env.)
  3. pip install requests
  4. python digikey_stock_check.py bom_schematic.csv  [bom_prototype.csv ...]

Writes <bom>_stock.csv next to each input and prints a summary table.
"""
import csv
import os
import sys
import time
import urllib.parse

import requests

from dotenv import load_dotenv
# reads .env from the current dir into os.environ
load_dotenv()

API = "https://api.digikey.com"
HEADERS_BASE = {
    "X-DIGIKEY-Locale-Site": "US",
    "X-DIGIKEY-Locale-Language": "en",
    "X-DIGIKEY-Locale-Currency": "USD",
}


def get_token(cid, secret):
    r = requests.post(
        f"{API}/v1/oauth2/token",
        data={"client_id": cid, "client_secret": secret, "grant_type": "client_credentials"},
        timeout=20,
    )
    r.raise_for_status()
    return r.json()["access_token"]


def headers(token, cid):
    return {**HEADERS_BASE, "Authorization": f"Bearer {token}", "X-DIGIKEY-Client-Id": cid}


def product_details(pn, h):
    """Exact lookup by MPN or DigiKey PN. Returns Product dict or None."""
    url = f"{API}/products/v4/search/{urllib.parse.quote(pn, safe='')}/productdetails"
    r = requests.get(url, headers=h, timeout=20)
    if r.status_code == 404:
        return None
    if r.status_code == 429:
        time.sleep(int(r.headers.get("Retry-After", "10")))
        return product_details(pn, h)
    r.raise_for_status()
    return r.json().get("Product")


def keyword_search(pn, h):
    """Return only an exact manufacturer-part-number match from keyword search."""
    r = requests.post(
        f"{API}/products/v4/search/keyword",
        headers={**h, "Content-Type": "application/json"},
        json={"Keywords": pn, "Limit": 5},
        timeout=20,
    )
    if not r.ok:
        return None
    prods = r.json().get("Products") or []
    for p in prods:  # prefer exact MPN match
        if p.get("ManufacturerProductNumber", "").upper() == pn.upper():
            return p
    return None


def matches_requested_part(product, mpn, dkpn):
    """Return whether a product exactly matches the requested MPN or DigiKey part number."""
    if product.get("ManufacturerProductNumber", "").upper() == mpn.upper():
        return True
    expected_dkpn = dkpn.upper()
    return any(
        variation.get("DigiKeyProductNumber", "").upper() == expected_dkpn
        for variation in product.get("ProductVariations") or []
    )


def positive_quantity(product):
    """Return a positive stock quantity, or zero when DigiKey reports none or no value."""
    try:
        return max(0, int(product.get("QuantityAvailable") or 0))
    except (TypeError, ValueError):
        return 0


def is_in_stock(product):
    """Return whether DigiKey marks the exact product active with positive inventory."""
    return (
        (product.get("ProductStatus") or {}).get("Status") == "Active"
        and positive_quantity(product) > 0
    )


def summarize(p):
    variations = p.get("ProductVariations") or []
    # Prefer cut tape / bulk / tube (small-qty friendly) for the DK part number
    small = [v for v in variations
             if (v.get("PackageType") or {}).get("Name", "").lower().startswith(("cut", "bulk", "tube", "tray"))]
    v = (small or variations or [{}])[0]
    return {
        "Found MPN": p.get("ManufacturerProductNumber", ""),
        "DK Part # (small qty)": v.get("DigiKeyProductNumber", ""),
        "Status": (p.get("ProductStatus") or {}).get("Status", ""),
        "Qty Available": p.get("QuantityAvailable", ""),
        "Unit Price": p.get("UnitPrice", ""),
        "Mfr Lead (wks)": p.get("ManufacturerLeadWeeks", ""),
        "URL": p.get("ProductUrl", ""),
    }


def check_bom(path, h):
    with open(path, newline="") as f:
        rows = list(csv.DictReader(f))
    out = []
    for row in rows:
        mpn = (row.get("Manufacturer Part Number") or "").strip()
        dkpn = (row.get("DigiKey Part Number") or "").split(" (")[0].strip()
        result = {"Found MPN": "", "DK Part # (small qty)": "", "Status": "NOT FOUND",
                  "Qty Available": "", "Unit Price": "", "Mfr Lead (wks)": "", "URL": ""}
        if mpn and not mpn.lower().startswith(("passives", "2.0in")):
            p = None
            for q in filter(None, [mpn, dkpn if dkpn not in ("N/A", "(verify)") else ""]):
                candidate = product_details(q, h)
                if candidate and matches_requested_part(candidate, mpn, dkpn):
                    p = candidate
                    break
            if not p:
                p = keyword_search(mpn, h)
            if p:
                result = summarize(p)
                if not is_in_stock(p):
                    result["Status"] = "OUT OF STOCK"
        else:
            result["Status"] = "SKIPPED"
        out.append({**row, **result})
        print(f"{mpn[:28]:28} {str(result['Status'])[:14]:14} "
              f"qty={str(result['Qty Available']):>8}  ${result['Unit Price']}  "
              f"lead={result['Mfr Lead (wks)']}")
        time.sleep(0.3)  # be gentle with rate limits

    out_path = os.path.splitext(path)[0] + "_stock.csv"
    with open(out_path, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(out[0].keys()))
        w.writeheader()
        w.writerows(out)
    print(f"-> wrote {out_path}\n")


def main():
    cid = os.environ.get("DIGIKEY_CLIENT_ID") or os.environ.get("CLIENT_ID")
    secret = os.environ.get("DIGIKEY_CLIENT_SECRET") or os.environ.get("CLIENT_SECRET")
    if not cid or not secret:
        sys.exit("Set DIGIKEY_CLIENT_ID and DIGIKEY_CLIENT_SECRET (or CLIENT_ID and CLIENT_SECRET)")
    files = sys.argv[1:] or ["bom_schematic.csv", "bom_prototype.csv"]
    h = headers(get_token(cid, secret), cid)
    for path in files:
        print(f"== {path}")
        check_bom(path, h)


if __name__ == "__main__":
    main()
