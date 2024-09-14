function sseHandler(event) {
  if (event?.detail?.type !== "topics-order-changed") return;

  const topicsList = document.getElementById("guild-topics");

  if (!topicsList) return;

  const sortableInstance = new Sortable(topicsList, {
    disabled: true,
    sort: false,
    animation: 400,
    filter: ".not-sortable",
    easing: "cubic-bezier(1, 0, 0, 1)",
  });

  let ids = JSON.parse(event.detail.data);

  setTimeout(() => {
    sortableInstance.sort(ids, true);
  }, 300);
}

htmx.onLoad(function () {
  document.body.removeEventListener("htmx:sseMessage", sseHandler);
  document.body.addEventListener("htmx:sseMessage", sseHandler);
});
