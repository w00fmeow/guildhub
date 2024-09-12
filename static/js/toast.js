function onToastEvent({ detail: { level, message } }) {
  Toastify({
    text: message,
    duration: 3000,
    className: level,
    offset: {
      x: "1em",
      y: "2em",
    },
  }).showToast();
}

document.body.addEventListener("show_toast", onToastEvent);

function onFailedRequestSend(ev) {
  const toastEvent = new CustomEvent("show_toast", {
    detail: {
      level: "error",
      message: "Failed to reach our servers. Please, try again later",
    },
  });

  document.body.dispatchEvent(toastEvent);
}

document.body.addEventListener("htmx:sendError", onFailedRequestSend);
