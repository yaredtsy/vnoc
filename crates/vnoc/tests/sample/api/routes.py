"""The handlers behind the routes."""


def get_order(service, number):
    return service.store.load(number)


def post_order(service, raw):
    return service.place(raw)
