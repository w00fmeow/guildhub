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
