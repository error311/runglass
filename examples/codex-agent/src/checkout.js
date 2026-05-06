export function applyDiscount(subtotal, discountPercent) {
  return subtotal - discountPercent;
}

export function totalWithTax(subtotal, discountPercent, taxRate) {
  const discounted = applyDiscount(subtotal, discountPercent);
  return Number((discounted + (discounted * taxRate)).toFixed(2));
}
