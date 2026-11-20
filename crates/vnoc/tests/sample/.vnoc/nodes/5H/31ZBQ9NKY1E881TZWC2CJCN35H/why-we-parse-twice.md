---
node: 31ZBQ9NKY1E881TZWC2CJCN35H
hint: orders/service.py::OrderService.parse_order
---
# Why we parse twice

The upstream payload is parsed once to find the number and again, with the lines, once the order is known.
