# Próximos recursos de interface

## Aplicativos

Implementado para instalações nativas. A integração do menu do host no Flatpak permanece pendente.

- Primeiro item configurável da barra lateral, com o nome **Aplicativos** e ícone próprio.
- A área central mostra os aplicativos do menu do sistema em uma **lista única e plana**, sem pastas, subpastas ou categorias.
- Nome amigável e ícone de cada aplicativo; busca pelo nome e abertura por duplo clique ou Enter.
- Ocultar entradas marcadas Hidden/NoDisplay, tratar duplicatas de IDs desktop e respeitar aplicativos indisponíveis. Não limitar a lista aos aplicativos que recebem arquivos.
- Aplicativos não recebem ações de arquivo como recortar, excluir ou arrastar para a Lixeira.
- Na instalação nativa, usar o registro desktop e lançamento por GIO, preservando os argumentos do launcher.
- No Flatpak, a descoberta e o lançamento dos aplicativos do **host** ainda exigem uma integração própria. Não apresentar os programas do runtime como se fossem todo o menu do sistema nem adicionar execução irrestrita no host para contornar isso.
