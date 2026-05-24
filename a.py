def calculate_total(items, tax_rate=0.05, discount=0.1):
    total = sum(item['price'] for item in items)
    return (total - discount) * (1 + tax_rate)

shopping_cart = [{'name': 'apple', 'price': 1.0}, {'name': 'banana', 'price': 0.5}]
print("Total:", calculate_total(shopping_cart))
