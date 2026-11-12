# Próximos recursos de interface

## Aparência das pastas

Implementado: cor e ícone por pasta em **Propriedades → Aparência**, com atalho no menu de contexto, persistência independente do tema e restauração individual do padrão.

## Aplicativos

Implementado para instalações nativas. A integração do menu do host no Flatpak permanece pendente.

- Primeiro item configurável da barra lateral, com o nome **Aplicativos** e ícone próprio.
- A área central mostra os aplicativos do menu do sistema em uma **lista única e plana**, sem pastas, subpastas ou categorias.
- Nome amigável e ícone de cada aplicativo; busca pelo nome e abertura por duplo clique ou Enter. Modos Detalhes, Lista e Ícones disponíveis, com modo e zoom salvos.
- Menu para abrir o gerenciador de software: Discover no KDE, GNOME Software no GNOME e um deles conforme disponibilidade no XFCE. Registros AppStream abrem detalhes; ausência de registro abre busca pelo nome.
- Ocultar entradas marcadas Hidden/NoDisplay, tratar duplicatas de IDs desktop e respeitar aplicativos indisponíveis. Não limitar a lista aos aplicativos que recebem arquivos.
- Aplicativos não recebem ações de arquivo como recortar, excluir ou arrastar para a Lixeira.
- Na instalação nativa, usar o registro desktop e lançamento por GIO, preservando os argumentos do launcher.
- No Flatpak, a descoberta e o lançamento dos aplicativos do **host** ainda exigem uma integração própria. Não apresentar os programas do runtime como se fossem todo o menu do sistema nem adicionar execução irrestrita no host para contornar isso.

## Ações específicas por tipo

Implementados: seção de compactados com preview/extração; rotação, redimensionamento e conversão de imagens estáticas com novas cópias; impressão por tipo com diálogo do sistema. As opções abaixo incluem sugestões para próximas etapas; impressão e as operações de imagem já estão disponíveis conforme documentado no README. Quando já houver preview correspondente, a proposta é permitir acessá-lo diretamente pela seção do tipo.

| Tipo | Sugestões |
| --- | --- |
| Imagens | Girar, redimensionar, converter, definir como plano de fundo |
| PDF | Imprimir, separar/juntar páginas, extrair texto |
| Documentos Word/Writer | Exportar PDF, imprimir |
| Planilhas Excel/Calc e CSV | Ver planilhas, exportar CSV/PDF, imprimir |
| Apresentações PowerPoint/Impress | Apresentar, exportar PDF/imagens |
| HTML e Markdown | Ver fonte/apresentação, exportar PDF, abrir HTML no navegador |
| Texto, código e scripts | Abrir no editor, comparar arquivos, escolher codificação; executar script no terminal apenas por ação explícita |
| Áudio | Reproduzir, ver/editar metadados, converter |
| Vídeo | Reproduzir, capturar quadro, extrair áudio, converter |
| ISO e imagens de disco | Montar para leitura, desmontar, abrir ferramenta de gravação |
| Pacotes de software | Abrir no gerenciador de software para revisar instalação/atualização/remoção |
| AppImage e lançadores | Gerenciar integração, permissões e informações do aplicativo |
| Certificados e chaves | Ver informações públicas, validade e impressão digital; importar certificado |
| Bancos de dados | Ver esquema/tabelas, exportar CSV, abrir no cliente disponível |
| Diagramas e desenhos | Exportar PNG/SVG/PDF, abrir no editor |
| Fontes | Ver amostra e instalar para o usuário |
| Agenda e contatos | Importar no calendário ou catálogo de contatos |
| Backups | Comparar com o original e restaurar para destino escolhido |
| Binários e tipos desconhecidos | Calcular SHA-256, ver metadados, abrir no editor hexadecimal |
| Pastas | Agrupar as ações existentes de terminal/compartilhamento e acrescentar compactação |

Ações aparecem conforme o tipo, a seleção e os componentes disponíveis. Próximas prioridades sugeridas: operações de páginas PDF e montagem de ISO. Conversões devem preservar o original e produzir um novo arquivo, com operações e confirmações apresentadas pelo componente responsável.
