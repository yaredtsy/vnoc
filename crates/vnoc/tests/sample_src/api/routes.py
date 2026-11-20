"""The handlers behind the routes."""


def get_order(service, number):
    return service.store.load(number)


def post_order(service, raw):
    return service.place(raw)


def legacy_export(service, since):
    """The old CSV export. Nothing calls it any more."""
    return [order.number for order in service.store.since(since)]
