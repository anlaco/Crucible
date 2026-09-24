// Selector de idioma del manual: un botón en la barra de mdBook que lleva a la
// misma página en el otro idioma.
//
// mdBook no sabe de idiomas, así que cada uno es un libro aparte en /es/ y
// /en/. Los capítulos se emparejan por su número, no por el nombre del
// fichero, para que cada idioma tenga URLs en su lengua. check.sh comprueba
// que esta tabla coincide con los ficheros de cada libro.
(function () {
  var CAPITULOS = {
    es: ["01-que-es", "02-instalacion", "03-primer-arranque", "04-conectar",
      "05-bancos", "06-perfiles", "07-modelos", "08-scpi", "09-receta",
      "10-referencia", "11-limitaciones", "12-problemas"],
    en: ["01-what-is", "02-installing", "03-first-start", "04-connecting",
      "05-benches", "06-profiles", "07-models", "08-scpi", "09-recipe",
      "10-reference", "11-limitations", "12-troubleshooting"]
  };
  var NOMBRE = { es: "Español", en: "English" };

  var m = location.pathname.match(/^(.*\/)(es|en)\/([^\/]*)$/);
  if (!m) return;
  var base = m[1], actual = m[2], pagina = m[3].replace(/\.html$/, "");

  function destino(idioma) {
    var i = CAPITULOS[actual].indexOf(pagina);
    var fichero = i < 0 ? "" : CAPITULOS[idioma][i] + ".html";
    // Sin el #ancla: los títulos, y con ellos los id, cambian de idioma.
    return base + idioma + "/" + fichero;
  }

  var barra = document.querySelector("#mdbook-menu-bar .right-buttons");
  if (!barra) return;
  Object.keys(NOMBRE).forEach(function (idioma) {
    if (idioma === actual) return;
    var a = document.createElement("a");
    a.href = destino(idioma);
    a.className = "icon-button";
    a.title = NOMBRE[idioma];
    a.setAttribute("aria-label", NOMBRE[idioma]);
    a.setAttribute("lang", idioma);
    a.textContent = idioma.toUpperCase();
    a.style.fontWeight = "600";
    a.style.fontSize = "0.9em";
    a.addEventListener("click", function () {
      try { localStorage.setItem("crucible-idioma", idioma); } catch (e) {}
    });
    barra.insertBefore(a, barra.firstChild);
  });
})();
