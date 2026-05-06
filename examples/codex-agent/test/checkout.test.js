import { strict as assert } from 'node:assert';
import test from 'node:test';

import { applyDiscount, totalWithTax } from '../src/checkout.js';

test('applyDiscount treats discount as a percentage', () => {
  assert.equal(applyDiscount(120, 25), 90);
});

test('totalWithTax applies discount before tax', () => {
  assert.equal(totalWithTax(120, 25, 0.0825), 97.42);
});
