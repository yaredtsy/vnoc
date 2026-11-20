"""The web application."""

from api.routes import get_order, post_order


def create_app(service):
    routes = {"GET /orders": get_order, "POST /orders": post_order}
    return {"service": service, "routes": routes}
