CREATE OR REPLACE VIEW product_profits AS
SELECT
    p.id,
    p.name,
    p.sale_price,
    p.purchase_price,
    p.sale_price - p.purchase_price AS profit,
    p.department_id
FROM products p;

