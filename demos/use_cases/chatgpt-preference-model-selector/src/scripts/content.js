(() => {
  const TAG = "[ModelSelector]";

  // Find the model selector button by visible label
  const findSelectorButton = () =>
    [...document.querySelectorAll('button')].find(
      btn => btn.textContent.match(/GPT|Default|4o/i)
    );

  let selectorButton = findSelectorButton();
  if (!selectorButton) {
    console.warn(`${TAG} Model selector button not found—will retry on DOM changes.`);
  } else {
    console.log(`${TAG} Listener attached to model selector.`);
    selectorButton.addEventListener('click', () => {
      console.log(`${TAG} Model selector clicked (dropdown opening).`);
    });
  }

  // Observe for late loads or UI updates
  const initObserver = new MutationObserver(() => {
    if (!selectorButton) {
      selectorButton = findSelectorButton();
      if (selectorButton) {
        console.log(`${TAG} (late) Listener attached to model selector.`);
        selectorButton.addEventListener('click', () => {
          console.log(`${TAG} Model selector clicked (dropdown opening).`);
        });
      }
    }
  });
  initObserver.observe(document.body, { childList: true, subtree: true });

  // Observe dropdown insertions
  const menuObserver = new MutationObserver(mutations => {
    for (const m of mutations) {
      for (const node of m.addedNodes) {
        if (
          node.nodeType === 1 &&
          node.querySelector &&
          node.querySelector('[role="menu"]')
        ) {
          console.log(`${TAG} Dropdown menu opened.`);
          node.querySelectorAll('[role="menuitem"]').forEach(item => {
            item.addEventListener('click', () => {
              console.log(`${TAG} Model selected →`, item.innerText.trim());
            });
          });
        }
      }
    }
  });
  menuObserver.observe(document.body, { childList: true, subtree: true });

  console.log(`${TAG} Content script injected.`);
})();
