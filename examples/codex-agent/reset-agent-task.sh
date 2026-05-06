#!/usr/bin/env sh
set -eu

mkdir -p src test bin

cat > package.json <<'JSON'
{
  "name": "runglass-codex-agent-example",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "test": "node --test"
  }
}
JSON

cat > src/checkout.js <<'JS'
export function applyDiscount(subtotal, discountPercent) {
  return subtotal - discountPercent;
}

export function totalWithTax(subtotal, discountPercent, taxRate) {
  const discounted = applyDiscount(subtotal, discountPercent);
  return Number((discounted + (discounted * taxRate)).toFixed(2));
}
JS

cat > test/checkout.test.js <<'JS'
import { strict as assert } from 'node:assert';
import test from 'node:test';

import { applyDiscount, totalWithTax } from '../src/checkout.js';

test('applyDiscount treats discount as a percentage', () => {
  assert.equal(applyDiscount(120, 25), 90);
});

test('totalWithTax applies discount before tax', () => {
  assert.equal(totalWithTax(120, 25, 0.0825), 97.42);
});
JS

rm -f .agent-plan.md
