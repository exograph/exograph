CREATE VIEW product_profits AS
SELECT
    p.id,
    p.name,
    p.sale_price,
    p.purchase_price,
    p.sale_price - p.purchase_price AS profit
FROM products p;

