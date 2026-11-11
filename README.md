# Linux System Explorer

[English](README.en.md)

**Versão experimental, em preparação para publicação.** O plano por formato e loja está
em [Distribuição](docs/DISTRIBUTION.md), com os passos de revisão em
[Release](docs/RELEASING.md). Ainda não há aprovação em lojas.

Explorador de arquivos desktop para Linux, escrito em Rust e inspirado na organização e na experiência do Windows Explorer. A primeira versão usa [egui/eframe](https://github.com/emilk/egui) com renderização OpenGL e suporte a Wayland e X11.

## Executar

Use Rust **1.95 ou superior**. Instale o Rust estável com [rustup](https://rustup.rs/). Na pasta do projeto:

```bash
cargo run
```

Para abrir uma pasta específica:

```bash
cargo run -- /caminho/da/pasta
```

Para gerar o executável otimizado:

```bash
cargo build --release
./target/release/linux-system-explorer
```

`--help` e `--version` funcionam sem abrir uma janela. Use `-- DIRETÓRIO` para um caminho
que comece com hífen.

## Preparar pacotes experimentais

Com Python 3.11+, `dpkg-dev`, `desktop-file-utils` e `appstream` instalados:

```bash
python3 scripts/package.py check
cargo build --locked --release
python3 scripts/package.py build
```

São gerados um arquivo `.tar.gz`, um pacote `.deb`, informações da compilação e
`SHA256SUMS` em `dist/`. Nada é instalado ou publicado automaticamente. Os pacotes
dependem da ABI do sistema usado na compilação; a base do workflow é Ubuntu 24.04.
A revisão das licenças e dos avisos de terceiros, vulnerabilidades e testes em sistemas
limpos ainda é necessária antes da redistribuição pública dos binários.

O Git local e o remoto SSH são preparados para `janderpt/linux-system-explorer`.
Consulte [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md) e
[CHANGELOG.md](CHANGELOG.md).

## Instalar pelo código do GitHub

Depois de instalar as dependências de compilação, Python 3.11+, `desktop-file-utils`
e `appstream`, em uma pasta para guardar o código:

```bash
git clone https://github.com/janderpt/linux-system-explorer.git
cd linux-system-explorer
cargo build --locked --release
python3 scripts/install-user.py install --set-default
```

A instalação fica em `~/.local/bin` e nos diretórios XDG do seu usuário, sem `sudo`.
O parâmetro `--set-default` torna o aplicativo o padrão para abrir pastas
(`inode/directory`). Sem ele, apenas instala. O Dolphin permanece disponível.
Os arquivos anteriores e as associações são preservados em um backup; o estado fica
em `~/.local/state/linux-system-explorer/install-state.json`.

Para retornar ao explorador anterior sem alterar outras associações:

```bash
python3 scripts/install-user.py restore-default
```

Com `--set-default`, o instalador também configura o serviço D-Bus
`org.freedesktop.FileManager1` para **Mostrar na pasta**: abre a pasta e seleciona os
itens solicitados, agrupando arquivos da mesma pasta na mesma janela. Também atende
**Abrir pasta** e **Propriedades**. Aceita URIs de arquivos locais; compartilhamentos
devem estar montados localmente. O serviço pode iniciar o explorador fechado. Um
serviço de outro gerenciador já ativo mantém a sessão atual até ser encerrado ou até
o próximo login. A restauração recupera também o registro D-Bus anterior, preservando
alterações posteriores feitas pelo usuário. A instalação de pacotes nativos não muda
essa preferência automaticamente.

Se uma janela desta versão já estiver na pasta solicitada, **Mostrar na pasta** a
reutiliza, atualiza a listagem, seleciona e revela os itens, limpa o filtro e solicita
o primeiro plano. Janelas em outras pastas permanecem onde estão. Sem uma janela
correspondente, abre uma nova. A pasta é conferida na própria janela, inclusive
durante uma navegação; janelas fechadas deixam de participar automaticamente.
No Wayland, a ativação usa o token enviado pelo aplicativo solicitante e respeita
a política de foco do compositor. Janelas de versões anteriores devem ser reabertas
após a atualização para participar dessa integração.

Abrir uma pasta pelo sistema (por exemplo, com `xdg-open` ou GIO) ou pelo executável
também reutiliza uma janela já naquela pasta. O novo processo encaminha o pedido e
o token de ativação e termina antes de criar outra interface. A seleção, o filtro
e o layout da janela são preservados. Abrir o aplicativo sem informar um caminho
aplica a mesma regra à pasta pessoal. Sem uma janela correspondente, abre normalmente.

É necessário um ambiente gráfico Linux com OpenGL e bibliotecas de X11 ou Wayland. A abertura de arquivos usa `gio open`, das ferramentas GIO, e os aplicativos associados no sistema. Se GIO não estiver instalado, usa `xdg-open`, normalmente fornecido pelo pacote `xdg-utils`. Em um ambiente mínimo Ubuntu/Debian, as dependências de compilação e execução podem ser instaladas com:

```bash
sudo apt install build-essential pkg-config libx11-dev libxi-dev libxcursor-dev libxrandr-dev libxkbcommon-dev libwayland-dev libegl1-mesa-dev xdg-utils libglib2.0-bin
```

## Abas, janelas e terminal integrado

O menu de contexto de pastas oferece **Abrir em nova aba**, **Abrir em nova janela** e **Abrir terminal aqui**, também nos atalhos da barra lateral e na área vazia da pasta corrente. As três ações têm ícones Font Awesome configuráveis.

A faixa é compacta, com abas encostadas e fundo opaco que acompanha o tema. O botão **+** tem formato de pequena aba e ícone configurável. As abas têm contorno próprio, ícone da pasta e botão de fechar integrado; a aba ativa recebe um fundo suave baseado na cor de destaque do sistema, com a mesma borda neutra das demais abas. A cor é detectada na paleta do KDE ou pelo portal de aparência do desktop, usando a paleta do aplicativo quando indisponível. Arraste uma aba para mudar sua posição: um marcador indica onde ela será inserida, e a faixa rola automaticamente nas bordas quando há muitas abas. Esc ou soltar fora da faixa cancela o arraste. A nova ordem é salva junto com a aba ativa e restaurada na próxima sessão, se a restauração estiver habilitada.

O clique direito na aba oferece **Duplicar aba**, **Fechar outras abas**, **Fechar abas à direita** e **Reabrir aba fechada**. A duplicação abre uma aba independente ao lado da original e a ativa, preservando pasta, histórico, filtro, seleção e visualização; operações e terminais continuam na aba original. As ações de fechamento usam a aba clicada como referência, preservam abas com operações de arquivos em andamento e ficam desabilitadas quando não há abas que possam fechar. As mudanças são salvas na sessão. Os ícones dessas ações podem ser personalizados em **Aparência e ícones**.

**Reabrir aba fechada** ou **Ctrl+Shift+T** recupera a última aba fechada e a ativa, incluindo pasta, posição, histórico, filtro, seleção e visualização. O submenu **Abas fechadas recentemente**, nas abas e no **+**, lista as 25 entradas em ordem de fechamento, da mais recente à mais antiga, com nome e caminho. Escolher uma entrada reabre apenas aquela aba e a remove do histórico; as demais continuam disponíveis. A lista tem rolagem e distingue entradas repetidas pelo contexto preservado. O clique direito no **+** também oferece a recuperação da última aba. O histórico mantém até 25 fechamentos por janela, incluindo fechamentos em grupo, e é salvo na sessão para continuar disponível após reinicializações. Pasta, posição relativa, histórico de navegação, filtro, seleção e layout são preservados. Ele também é carregado ao iniciar na pasta pessoal ou abrir uma pasta específica. Com várias janelas, o histórico da última janela alterada ou fechada acompanha a sessão. Terminais e operações encerrados não são reiniciados. A seleção é conferida novamente quando a pasta é lida.

Cada aba mantém sua pasta, histórico, seleção, filtro, rolagem, modo de exibição e zoom. **+** ou **Ctrl+T** cria uma aba na pasta pessoal; **Ctrl+Tab** e **Ctrl+Shift+Tab** alternam entre abas; **×** ou **Ctrl+W** fecha a aba ativa. Operações de arquivos continuam nas abas em segundo plano; copiar e colar e os avisos são compartilhados entre abas da mesma janela. Uma aba com uma operação de arquivos em andamento permanece aberta até a conclusão. As preferências gerais, como tema e idioma, valem para todas as abas. Os layouts por pasta continuam sendo salvos. As pastas das abas, sua ordem e a aba ativa são gravadas automaticamente e restauradas ao iniciar o aplicativo sem indicar uma pasta. Em **Configurações → Geral → Ao iniciar**, escolha **Restaurar abas da última sessão** (padrão) ou **Iniciar sempre na pasta pessoal**. A escolha vale para o aplicativo inteiro e entra em vigor na próxima abertura.

A sessão fica em `$XDG_STATE_HOME/linux-system-explorer/session.json` (por padrão, `~/.local/state/linux-system-explorer/session.json`), separada das preferências. Abas repetidas e nomes de pasta não UTF-8 são preservados. Com várias janelas, é retomada a última janela utilizada ou fechada. Abrir uma pasta específica, usar **Mostrar na pasta** ou **Abrir em nova janela** mantém o destino solicitado e não carrega abas de outra sessão. Pastas indisponíveis continuam em suas abas, podendo ser atualizadas com **F5** quando voltarem a estar disponíveis.

**Abrir em nova janela** sempre cria uma janela independente, mesmo que a pasta já esteja aberta. O equivalente no terminal é `linux-system-explorer --new-window -- /caminho/da/pasta`. A abertura comum pelo sistema e **Mostrar na pasta** continuam procurando uma pasta já aberta, inclusive em abas inativas; nesse caso, ativam a aba correspondente e solicitam o primeiro plano.

**Abrir terminal aqui** mostra um painel inferior redimensionável, abaixo da área de exploração, reduzindo a altura dela. É um terminal integrado com pseudoterminal real, iniciado na pasta escolhida e usando o shell do usuário (`SHELL`, cadastro do usuário ou `/bin/sh`). A navegação da aba e os comandos `cd` do terminal são independentes. O painel suporta entrada interativa, cores ANSI, histórico de saída com rolagem, seleção de texto, **Ctrl+Shift+C** para copiar, colagem e sinais como **Ctrl+C**. Atalhos de arquivos, como Delete e copiar, não são executados enquanto o terminal tem o foco.

**F4** ou **×** oculta o painel sem encerrar os comandos. Abrir o terminal em outra pasta mantém a sessão anterior disponível no seletor do painel. `exit` encerra a sessão; fechar a aba ou a janela fecha seus terminais. As sessões não são restauradas ao reiniciar. O painel usa seu próprio emulador; não incorpora os perfis nem a interface do Konsole ou de outro aplicativo de terminal.

## Primeira versão

- Pastas reais do sistema, com leitura em uma thread dedicada.
- Voltar, avançar, subir um nível e atualizar.
- Toolbar única na primeira linha da interface, acima da barra lateral e da área de arquivos; breadcrumb e filtro à direita permanecem no corpo da pasta.
- Caminho por segmentos clicáveis e campo de endereço com suporte a `~`, acessível pelo ícone de edição ao lado de atualizar.
- Botão para sair da edição do caminho e seletor de pastas no final do campo, com navegação por subpastas e confirmação.
- Breadcrumb adaptável: prioriza a pasta atual, abrevia nomes longos e reúne os ancestrais que não couberem no menu `…`; o caminho completo aparece ao passar o mouse.
- Barra da pasta com navegação, nova pasta, copiar, recortar, colar, itens ocultos e modos de exibição. Em janelas estreitas, as ações excedentes ficam em **Mais ações da pasta**.
- Configurações e tema no extremo direito da toolbar, sem repetir o nome e o ícone da aplicação no corpo da janela.
- Ícone de explorador consistente na janela, no lançador e na barra de tarefas.
- Acesso rápido às pastas do usuário conforme a configuração XDG, com ícones específicos para cada local.
- Destaque do ancestral visível mais próximo da pasta aberta, incluindo atalhos, pastas fixadas e compartilhamentos de rede.
- Aba **Barra lateral** para escolher atalhos e seções, com pastas fixadas e atalhos de rede persistentes.
- Ícones Font Awesome configuráveis nos menus de contexto e nas ações correspondentes da barra.
- Ícones por família de arquivo: PDF, documentos, planilhas, apresentações, imagens, mídia, arquivos compactados, código, instaladores, certificados e outros formatos conhecidos.
- Visualizações de detalhes, lista compacta e ícones, com renderização de linhas visíveis.
- Botões para diminuir e aumentar os itens, de 75% a 200%, com tamanho salvo entre execuções.
- Pesquisa pelo nome na pasta atual, sem diferenciar maiúsculas e minúsculas.
- Ordenação pelos cabeçalhos das colunas, com cliques sucessivos alternando o sentido e pastas sempre primeiro.
- Janela de configurações com 25 colunas possíveis e rolagem horizontal na lista.
- Redimensionamento pelas divisórias dos cabeçalhos e reordenação arrastando os cabeçalhos, com largura, ordem e modo de visualização salvos por pasta.
- Ícones Font Awesome Free incorporados: modo monocromático ou colorido, tamanho e escolha do ícone de cada ação/local.
- Preferências de colunas, ícones, tema, visualização, ordenação e arquivos ocultos salvas entre execuções.
- Exibição opcional de arquivos ocultos.
- Seleção múltipla com Ctrl+clique e Shift+clique nos três modos, duplo clique para abrir e menu de contexto.
- Contagem e tamanho total dos itens selecionados, com cópia de todos os caminhos marcados.
- Copiar, recortar e colar arquivos e pastas em segundo plano, com progresso, cancelamento e resolução de conflitos sem sobrescrita.
- Envio dos selecionados à lixeira com Del e exclusão permanente com Shift+Del, mediante confirmação.
- Lixeira configurável na barra lateral, com caminho original, data de exclusão e restauração de seleções pelo menu de contexto.
- **Esvaziar lixeira** no menu de contexto do atalho, com confirmação obrigatória e exclusão permanente em segundo plano.
- Abertura de arquivos no aplicativo padrão do Linux.
- Criação de pastas, cópia do caminho e propriedades básicas.
- Temas claro e escuro; interface em português e inglês, seguindo o idioma do sistema por padrão.

## Atalhos

| Atalho | Ação |
| --- | --- |
| `Alt` + `←` | Voltar |
| `Alt` + `→` | Avançar |
| `Alt` + `↑` | Pasta acima |
| `Ctrl` + `L` | Editar caminho |
| `Ctrl` + `F` | Focar pesquisa |
| `Ctrl` + `H` | Mostrar ou ocultar itens ocultos |
| `Ctrl` + `Shift` + `N` | Criar pasta |
| `Ctrl` + `C` | Copiar arquivos e pastas selecionados |
| `Ctrl` + `X` | Recortar arquivos e pastas selecionados |
| `Ctrl` + `V` | Colar na pasta atual |
| `Del` | Em pastas, enviar à lixeira; na Lixeira, excluir permanentemente com confirmação obrigatória |
| `Shift` + `Del` | Excluir permanentemente os selecionados, com confirmação |
| `Ctrl` + clique | Adicionar ou remover um item da seleção |
| `Shift` + clique | Selecionar o intervalo desde a âncora, na ordem e no filtro exibidos |
| `Shift` + `↑` / `↓` | Estender ou reduzir a seleção até o item anterior/próximo |
| `Page Up` / `Page Down` | Mover a seleção por uma página de itens, conforme a altura visível, zoom e modo |
| `Home` / `End` | Ir ao primeiro/último item da ordem exibida |
| `Shift` + `Page Up` / `Page Down` / `Home` / `End` | Estender ou reduzir a seleção desde a âncora |
| `Ctrl` + `Shift` + clique | Acrescentar um intervalo à seleção existente |
| `F2` | Renomear o único item selecionado |
| `F5` | Atualizar |
| `Esc` | Sair da edição do caminho, limpar a pesquisa ou cancelar a criação |

## Organização

```text
src/
  main.rs         Inicialização da janela e caminho inicial
  app.rs          Interface, estado e comunicação com a thread de leitura
  app/rename.rs   Diálogo e atualização de referências após renomear
  renames.rs      Renomeação atômica sem sobrescrever
  app/drag_drop.rs Seleção arrastada e resolução do destino
  native_dnd.rs   Integração nativa de arquivos com Wayland e X11
  applications.rs Aplicativos instalados, consulta MIME e abertura via GIO
  app/open_with.rs Seletor de aplicativo com busca e abertura em segundo plano
  app/breadcrumbs.rs  Abreviação e navegação do caminho
  app/folder_picker.rs  Seleção de pastas em segundo plano
  system_style.rs  Ícone nativo gerado a partir do SVG do aplicativo
  file_manager.rs  Serviço D-Bus Mostrar na pasta e seleção externa
  desktop.rs      Registro da identidade desktop para Wayland e lançadores
  i18n.rs         Detecção de idioma, traduções, plurais e formatos
  app/table.rs    Cabeçalhos redimensionáveis, reordenação e lista compacta
  app/sidebar.rs  Seções, destaque do ancestral e pastas fixadas
  app/toolbar.rs  Ações da pasta e menu de opções excedentes
  app/transfers.rs  Área de transferência e interface das operações
  app/trash.rs     Restauração de seleções e interface da lixeira
  app/empty_trash.rs  Confirmação, progresso e cancelamento ao esvaziar a lixeira
  trashbin.rs      Listagem da lixeira, metadados e restauração sem sobrescrita
  transfers.rs    Cópia, movimentação, conflitos e cancelamento
  app/deletions.rs  Atalhos, confirmação e progresso de exclusão
  deletions.rs     Lixeira e remoção permanente dos selecionados
  columns.rs      Colunas, apresentação e comparação dos metadados
  icons.rs        Catálogo e renderização dos ícones Font Awesome
  filetypes.rs    Classificação dos ícones por formato e nome de arquivo
  settings.rs     Configurações, validação e persistência
  sidebar.rs      Atalhos, persistência e detecção de montagens de rede
  filesystem.rs   Leitura, filtragem, ordenação e validação de nomes
  navigation.rs   Histórico de navegação
```

O histórico só é alterado depois de uma leitura bem-sucedida. Falhas de acesso preservam a pasta anterior. Links simbólicos para pastas podem ser navegados; links quebrados continuam visíveis. Os caminhos reais são mantidos como `PathBuf`, inclusive quando seus nomes não são UTF-8; a interface usa uma representação legível desses nomes.

A busca cobre apenas os nomes na pasta atual. O tamanho de pastas não é calculado recursivamente. A atualização de alterações externas é manual; transferências e exclusões feitas pelo aplicativo atualizam a listagem automaticamente. A primeira versão ainda não inclui abas, miniaturas ou montagem de volumes.

Um clique simples seleciona apenas o item clicado. Ctrl+clique alterna a seleção individual e estabelece uma nova âncora. Shift+clique marca o intervalo entre a âncora e o item clicado; novos cliques com Shift continuam usando a mesma âncora. Shift+↑ e Shift+↓ avançam o cursor pela ordem exibida e estendem ou reduzem o intervalo desde a mesma âncora. Page Up e Page Down movem a seleção por uma página de itens, considerando a altura disponível, o nível de zoom e o número de colunas no modo Ícones. Home e End vão ao primeiro e ao último item da ordem filtrada e ordenada. Com Shift, essas quatro teclas estendem ou reduzem o intervalo desde a âncora. A rolagem acompanha o cursor, inclusive fora da área inicialmente visível. Nos campos de edição, essas teclas continuam selecionando texto. O clique direito em um item já marcado preserva o grupo, e **Copiar caminhos selecionados** copia um caminho por linha, na ordem exibida. Alterar o filtro remove da seleção os itens que ficaram ocultos; atualizar a pasta mantém os itens que ainda existem, e navegar para outra pasta limpa a seleção.

A toolbar, a linha do caminho/filtro e o cabeçalho da grid compartilham a mesma altura: 40 px com o tamanho padrão dos ícones. A altura acompanha ícones maiores para manter o alinhamento. A toolbar separa navegação e ações por um espaço, sem linha vertical, e tem folga vertical, e o campo de filtro permanece centralizado à direita. Os avisos operacionais, incluindo falhas ao abrir pastas e aplicativos ou ao iniciar operações de arquivos, aparecem em toasts empilhados no canto inferior direito, sem deslocar a lista. Cada mensagem, inclusive mensagens repetidas, tem seu próprio botão **×** e sua contagem de tempo; um novo aviso não substitui os anteriores. Os avisos usam verde e confirmação para sucesso, amarelo e triângulo para cancelamentos ou itens ignorados, e vermelho e símbolo de erro quando houver falhas, incluindo o primeiro erro na mensagem. Os três ícones Font Awesome podem ser personalizados em **Aparência e ícones**; as cores de estado são mantidas mesmo no modo de ícones em preto e branco. Os mais recentes ficam na base da pilha. Pilhas extensas têm rolagem, permitindo consultar e fechar os avisos antigos sem ultrapassar a janela. Em **Configurações → Geral → Notificações**, configure separadamente **Sucesso**, **Aviso** e **Erro**: tempo entre 1 e 300 segundos ou **Fechamento manual**. Por padrão, sucesso e aviso duram seis segundos, enquanto erros permanecem até o fechamento manual. As opções são globais e salvas entre execuções. Preferências antigas de duração e fechamento são preservadas para sucesso e aviso; erros começam com fechamento manual. Alterar um tipo reinicia apenas os temporizadores dos avisos daquele tipo. Os botões **Testar sucesso**, **Testar aviso** e **Testar erro** mostram uma prévia com os valores de duração e fechamento ainda em edição, sem aplicar nem salvar as preferências. Os ícones também usam as escolhas ainda em edição; as cores de estado seguem o tema atual. No modo automático, a contagem é renovada apenas para o aviso sob o ponteiro. No modo manual, o toast permanece visível e iniciar outra operação não o esconde; novos avisos são acrescentados à pilha. O botão **×** fecha o aviso em qualquer modo. A opção **Fechar todos** aparece apenas quando há mais de um aviso; fica no topo da pilha, acessível durante a rolagem, e dispensa todos os avisos de uma vez. O progresso de operações em andamento continua disponível na área de arquivos.

## Renomear e arrastar arquivos

Selecione um arquivo ou pasta e use **F2** ou **Renomear** no menu de contexto. O diálogo inicia com o nome atual; para arquivos, seleciona o nome sem a última extensão, enquanto pastas têm o nome inteiro selecionado. **Enter** ou **Renomear** confirma; **Cancelar** ou Esc preserva o nome. A operação é feita em um item por vez, em segundo plano. Nomes inválidos e conflitos são informados sem sobrescrever o destino, incluindo links quebrados que já ocupem esse nome. Um link simbólico é renomeado sem alterar seu destino. Itens na Lixeira precisam ser restaurados antes.

A seleção acompanha o novo nome. Ao renomear uma pasta, o histórico, os atalhos fixados e suas preferências de visualização (inclusive de subpastas) passam a usar o novo caminho. Os caminhos correspondentes no buffer de copiar/recortar também são atualizados.

Nos três modos de visualização, arraste um ou vários itens selecionados para uma pasta visível, para a área vazia da pasta aberta ou para um atalho da barra lateral, incluindo pastas fixadas e compartilhamentos montados. Arrastar dentro do explorador **move** os itens; mantenha **Ctrl** para **copiar**. O destino é destacado e a lista rola ao aproximar o ponteiro das bordas. Esc cancela o gesto antes de soltar. Soltar na própria pasta de origem é ignorado; mover uma pasta para dentro dela mesma é rejeitado. Os conflitos, o progresso e o cancelamento usam o mesmo mecanismo das transferências por copiar/colar.

Também há troca de arquivos com outros aplicativos: arrastar para fora exporta arquivos locais como `text/uri-list`; soltar arquivos vindos do sistema usa a pasta ou atalho apontado, ou a pasta atual quando o alvo é sua área vazia. **Ctrl solicita copiar** e **Shift solicita mover**, nos dois sentidos, conforme as operações aceitas pelo outro aplicativo. Ctrl tem prioridade se as duas teclas estiverem pressionadas. Sem modificadores, a entrada prefere copiar; na saída, o destino participa da negociação padrão do sistema. No X11, a preferência é atualizada durante o gesto e a posição/ação do drop ficam fixadas ao soltar. No Wayland, o compositor negocia as ações e os modificadores. Uma movimentação recebida só é confirmada ao aplicativo de origem depois de a transferência inteira terminar; falhas parciais e cancelamentos não enviam confirmação de sucesso. Na saída, o destino executa a movimentação e o explorador atualiza a lista sem excluir os arquivos novamente. Os cursores vêm do tema do SO: seta para movimentação, indicador de cópia e sinal de destino inválido. No X11, a integração usa XDND v5 com um proxy para receber a posição nativa do drop, URIs locais e transferência incremental para seleções extensas. No Wayland, usa `guth-wayland-dnd` na conexão da própria janela, que exige Rust 1.95+. Aplicativos de destino precisam aceitar arquivos locais. Não são importados URLs de rede nem dados arbitrários de páginas web. A Lixeira recebe itens com a confirmação configurada para envio à lixeira; arrastar itens de dentro dela requer restaurá-los primeiro.

## Copiar, recortar e colar

Selecione os itens e use os atalhos, os ícones da barra da pasta ou **Copiar/Recortar/Colar** no menu de contexto. O menu de uma área vazia oferece **Colar** na pasta atual. Ao usar **Colar** no menu de uma pasta, ela recebe os itens diretamente.

As operações funcionam entre pastas desta instância do explorador. O buffer de arquivos é mantido durante a navegação, permite repetir uma cópia e é limpo após uma movimentação concluída. Em falhas parciais, os itens não movidos continuam disponíveis para outra tentativa. A integração com formatos nativos de arquivos das áreas de transferência de outros gerenciadores ainda não é implementada; os caminhos também são exportados como texto. Em campos de edição, os atalhos continuam operando sobre texto, e **Copiar caminhos selecionados** permanece uma ação separada.

Pastas são copiadas recursivamente, incluindo itens ocultos. Links simbólicos são reproduzidos sem percorrer seus destinos. As cópias preservam permissões POSIX e datas de acesso/modificação; proprietário/grupo, ACLs, atributos estendidos e vínculos entre hard links não são replicados. Copiar sockets, FIFOs ou dispositivos gera erro por item, sem bloquear o restante da operação.

Conflitos oferecem **Manter ambos**, **Ignorar** e **Cancelar transferência**, com opção de aplicar a escolha aos próximos conflitos. **Manter ambos** gera nomes como `arquivo (cópia).txt`. Os itens existentes não são sobrescritos. Não é permitido colar uma pasta dentro dela própria, inclusive por um caminho que passe por link simbólico.

O recorte só movimenta os itens ao colar. No mesmo sistema de arquivos, usa renomeação sem sobrescrita. Entre sistemas de arquivos, a cópia é preparada e verificada antes da publicação do destino e da remoção dos dados copiados da origem. Cancelamento ou falha durante a cópia mantém a origem e remove a preparação temporária. Se houver falha na remoção da origem após a cópia, ela é reportada, e o destino completo é preservado. Fechar a janela durante uma transferência solicita o cancelamento e aguarda sua finalização.

## Edição do caminho e seleção de pasta

**Ctrl+L** ou o lápis abre a edição do caminho. O botão **×** à esquerda do campo sai desse modo, descarta o texto ainda não aberto e restaura o breadcrumb da pasta ativa; **Esc** faz o mesmo. **Enter** continua abrindo o caminho digitado.

No final do campo, o botão com ícone de árvore de pastas abre **Selecionar pasta**. O popup inicia no caminho digitado, aceita a navegação por subpastas e oferece subir um nível, pasta pessoal, sistema de arquivos, atualização e ancestrais clicáveis. As leituras ocorrem em segundo plano e respeitam a opção de arquivos ocultos. **Usar esta pasta** muda a pasta ativa; **Cancelar** ou **Esc** fecha o popup e preserva o texto em edição. Explorar o popup não altera a seleção nem o histórico da janela principal até confirmar.

O ícone da janela, do lançador e da barra de tarefas deriva de um único SVG do projeto. A compilação gera o PNG transparente de 256 px usado pela janela e instalado no tema hicolor junto com o SVG. Assim, os protocolos de janela e a descoberta pelo desktop usam o mesmo desenho e as mesmas cores. Os botões do caminho têm ícones personalizáveis em **Aparência e ícones**.

No Wayland, o gerenciador de janelas obtém o ícone pela identidade desktop `io.github.janderpt.linux-system-explorer`, também usada pelo lançador e pelo nome do ícone. Instalações empacotadas usam os arquivos instalados pelo pacote, sem regravá-los ao abrir. Em builds de desenvolvimento sem lançador instalado, o explorador registra um atalho identificado como gerado e o mesmo PNG em `$XDG_DATA_HOME/icons/hicolor/256x256/apps`. O atalho gerado é removido quando há um lançador instalado, preservando atalhos personalizados. O instalador por usuário atualiza os caches de ícones e aplicativos do KDE. Janelas já abertas devem ser reabertas após a atualização. Os detalhes estão em [Distribuição](docs/DISTRIBUTION.md).

## Lixeira e exclusão permanente

**Del** pede confirmação antes de enviar todos os arquivos e pastas selecionados à lixeira do Linux, seguindo o padrão FreeDesktop. Em **Configurações → Geral**, a opção **Pedir confirmação antes de enviar itens à lixeira** permite desativar essa pergunta; ela vem ativada por padrão, inclusive ao carregar preferências de versões anteriores, e é salva entre execuções para todas as pastas. A mesma escolha vale para o menu de contexto. **Shift+Del** sempre abre uma confirmação com a quantidade e os nomes dos itens antes de removê-los permanentemente. **Cancelar** ou **Esc** fecha a confirmação sem excluir. As duas ações também aparecem no menu de contexto, com ícones Font Awesome personalizáveis. Em campos de edição, Delete continua operando sobre texto.

As operações são executadas em segundo plano e atualizam a pasta ao terminar. Uma falha é reportada por item, sem impedir os demais; se o envio à lixeira falhar, o aplicativo nunca o substitui por exclusão permanente. Links simbólicos são removidos como links, preservando seus destinos. Os itens concluídos são retirados da seleção e do buffer de copiar/recortar; itens que falharam permanecem disponíveis quando ainda existem.

É possível interromper a operação entre os itens selecionados. A remoção de uma pasta já iniciada é concluída antes da interrupção; falhas durante uma exclusão permanente podem deixar parte do conteúdo removida. Fechar a janela solicita essa interrupção e aguarda o item em andamento. Transferências e exclusões não são executadas simultaneamente. A restauração pode ser feita diretamente pela localização **Lixeira** na barra lateral.

### Visualizar e restaurar a lixeira

**Lixeira** aparece em **Este computador** na barra lateral. A opção **Configurações → Barra lateral → Exibir Lixeira** controla a visibilidade e é salva entre execuções. Também é possível abrir `trash:///` pelo campo de caminho.

A localização reúne os itens da lixeira do usuário, incluindo lixeiras dos volumes montados reconhecidas pelo sistema. A visualização inicial em Detalhes mostra **Nome**, **Caminho original**, **Data de exclusão**, **Tipo** e **Tamanho**. Nomes repetidos continuam sendo itens distintos. Nomes iniciados por ponto permanecem visíveis nessa localização. F5 atualiza a listagem; filtro, ordenação, seleção múltipla, modos de exibição e zoom continuam disponíveis. A Lixeira mantém seu próprio layout e zoom e participa do histórico Voltar/Avançar.

Selecione um ou vários arquivos e pastas, clique com o botão direito e escolha **Restaurar**. A restauração acontece em segundo plano para o caminho original e recria pastas de origem ausentes. Links simbólicos são restaurados como links. **Manter ambos**, **Ignorar** e **Cancelar restauração** resolvem conflitos; manter ambos gera nomes como `arquivo (restaurado).txt` ou `file (restored).txt`, conforme o idioma. Os destinos existentes são preservados, inclusive se surgirem durante a operação.

Falhas são reportadas por item, permitindo concluir os demais. Após a restauração, os itens concluídos saem da lixeira e da seleção. É possível cancelar uma operação e fechar a janela aguardando o encerramento do trabalho em andamento. O menu também oferece **Copiar caminho original** e **Propriedades**. Os ícones da Lixeira e de Restaurar são personalizáveis em **Aparência e ícones**. A visualização da lixeira usa ações próprias de restauração e exclusão permanente; copiar/recortar/colar, criar pastas e enviar novamente à lixeira ficam desabilitados nessa localização.

Na **Lixeira**, **Del**, **Shift+Del** ou **Excluir permanentemente…** no menu de contexto excluem apenas os itens selecionados. A confirmação é sempre obrigatória, mesmo com a confirmação do envio à lixeira desativada. Ela mostra os nomes originais numa lista com rolagem. A exclusão remove os dados e seus registros da lixeira em segundo plano; os itens não selecionados permanecem disponíveis. **Cancelar** ou **Esc** preserva os itens.

### Esvaziar a lixeira

Clique com o botão direito sobre **Lixeira** na barra lateral e escolha **Esvaziar lixeira**. A confirmação carrega os itens em segundo plano e mostra a quantidade e uma lista de altura fixa com rolagem. **Cancelar** ou **Esc** fecha a confirmação sem excluir; **Esvaziar lixeira** exclui permanentemente os itens listados, incluindo o conteúdo das pastas. A confirmação é obrigatória mesmo quando a pergunta do Del está desativada.

A operação usa a lista confirmada: itens enviados à lixeira depois dessa listagem são preservados. Links simbólicos são removidos como links e seus destinos permanecem intactos. Falhas são reportadas por item e os demais continuam sendo processados. Itens que não puderam ser listados são preservados e essa situação é informada. **Interromper após este item** cancela os próximos itens; fechar a janela aguarda a conclusão do item em andamento. Restaurações, transferências e outras exclusões não são executadas em paralelo com o esvaziamento. O ícone da ação pode ser escolhido em **Aparência e ícones**.

## Configurações e metadados Linux

A engrenagem abre as abas **Geral**, **Barra lateral**, **Colunas** e **Aparência e ícones**, nessa ordem, começando por **Geral**. Todas mantêm a mesma altura, com rolagem independente no conteúdo de cada aba e botões em posições estáveis. Os elementos são centralizados verticalmente em suas linhas. **Aplicar** salva as alterações e mantém a janela aberta; **Salvar e fechar** também fecha a janela. **Cancelar** descarta as alterações ainda não aplicadas. O catálogo permite buscar pelo nome Font Awesome (por exemplo, `house`, `folder` ou `gear`) e escolher o ícone de cada uma das 79 ações, locais e notificações configuráveis. São 1.407 ícones do estilo Free Solid, incorporados ao executável e disponíveis sem conexão. A licença e a origem estão em [assets/fontawesome](assets/fontawesome/README.md). Os menus de contexto usam essas mesmas escolhas para Abrir, Abrir com, Copiar, Recortar, Colar, Renomear, Copiar caminho, Propriedades, Fixar, Desafixar e as duas ações de exclusão.

### Ícones de arquivos e abertura no aplicativo padrão

Os três modos de visualização e a janela de propriedades usam ícones conforme a família do arquivo. Em modo colorido, PDF usa vermelho, documentos Word/Writer usam azul, planilhas e CSV usam verde e apresentações usam laranja. Imagens, áudio, vídeo, arquivos compactados, código, texto, pacotes, executáveis, certificados e chaves, bancos de dados, discos, diagramas, fontes, binários, backups e calendários também têm ícones próprios. **Aparência e ícones** permite personalizar cada categoria, mantendo a escolha entre coloridos e monocromáticos.

A classificação usa a extensão sem diferenciar maiúsculas de minúsculas, nomes conhecidos como Dockerfile e README e os bits de execução do Linux. Nomes desconhecidos usam o ícone genérico. Na Lixeira, a classificação usa o nome original. Pastas e marcadores de links simbólicos continuam sendo reconhecidos.

**Duplo clique** e **Abrir** no menu de contexto usam a mesma abertura pelo `gio open` (com fallback para `xdg-open` se GIO não estiver instalado), seguindo o aplicativo associado pelo Linux ao tipo de arquivo. Pastas são abertas dentro do explorador. O caminho é passado como um único argumento, inclusive para nomes com espaços, caracteres especiais e nomes não UTF-8. Falhas ao executar o abridor ou ao abrir o item são exibidas na interface, incluindo os diagnósticos enviados pela ferramenta. Para abrir um item que está na Lixeira, restaure-o primeiro.

**Abrir com…** no menu de contexto de um arquivo abre um seletor com busca, lista de altura fixa e rolagem. O aplicativo padrão aparece primeiro, seguido pelos aplicativos que declaram suporte ao tipo MIME. **Mostrar outros aplicativos** inclui os demais aplicativos instalados que aceitam arquivos; essa lista também é exibida quando não há recomendados. Selecione um aplicativo e clique em **Abrir**, ou dê duplo clique nele. **Cancelar** ou Esc fecha o seletor. A abertura vale para o arquivo clicado, mesmo quando houver uma seleção múltipla. A opção **Usar sempre este aplicativo para este tipo de arquivo** começa desmarcada. Quando marcada, **Abrir** (ou o duplo clique no aplicativo) salva a associação padrão do sistema para o tipo MIME identificado, usando `gio mime`, e abre o arquivo. A escolha também passa a valer nas próximas aberturas pelo explorador e por outros aplicativos do usuário. Sem marcar, a abertura é pontual. Marcar a opção sozinho, **Cancelar** ou Esc não grava alterações. Quando o tipo MIME não puder ser identificado, a opção fica desabilitada. Se a associação não puder ser gravada, o erro é exibido e a abertura não prossegue.

Os aplicativos são descobertos nos registros `.desktop` de `$XDG_DATA_HOME/applications` e `$XDG_DATA_DIRS`, com prioridade para as configurações do usuário, nomes localizados e verificação dos executáveis disponíveis. A consulta acontece em segundo plano. A abertura usa `gio launch`, das ferramentas GIO (pacote `libglib2.0-bin` no Ubuntu/Debian), para interpretar os lançadores do sistema, incluindo campos de arquivos e URIs. Falhas são informadas na interface. O ícone de **Abrir com** pode ser personalizado em **Aparência e ícones**.

### Idioma e tamanho dos itens

Em **Geral → Idioma**, escolha **Seguir idioma do sistema**, **Português** ou **English**. A escolha manual é global, fica salva e entra em vigor ao usar **Aplicar** ou **Salvar e fechar**, incluindo o título da janela, sem reiniciar. Preferências anteriores usam a opção de seguir o sistema.

A detecção lê `LC_ALL`, `LC_MESSAGES` e `LANG`, nessa prioridade, e considera a lista de idiomas preferidos de `LANGUAGE` quando a sessão não está em `C`/`POSIX`. Variantes `pt` e `en` são reconhecidas; idiomas não suportados usam inglês. O idioma do sistema é identificado na inicialização. As traduções são incorporadas a partir de `locales/messages.json`, com mensagens completas e plurais. Nomes de arquivos, caminhos e nomes personalizados de atalhos permanecem intactos. Mensagens emitidas pelo próprio sistema operacional ou por ferramentas externas podem continuar no idioma dessas ferramentas.

Datas usam `dd/mm/aaaa HH:mm` em português e `aaaa-mm-dd HH:mm` em inglês. O fuso horário continua sendo o do sistema. Tamanhos fracionários usam vírgula em português e ponto em inglês; os identificadores de arquivos e os valores usados na ordenação permanecem preservados.

Na barra da pasta, as lupas com **−** e **+** diminuem ou aumentam ícones, texto e altura dos itens em passos de 25%, entre 75% e 200%, nas três visualizações. Em janelas estreitas, ficam acessíveis no menu de ações excedentes. O valor também pode ser escolhido em **Geral → Tamanho dos itens desta pasta**. O zoom é salvo por pasta junto com o modo de visualização, sendo restaurado ao navegar, usar Voltar/Avançar ou reabrir o aplicativo; as larguras de colunas e a seleção são preservadas. Preferências anteriores mantêm o antigo zoom global como padrão das pastas ainda não personalizadas e dos perfis que ainda não tinham zoom próprio.

### Barra lateral e rede

Em **Configurações → Barra lateral**, escolha quais atalhos padrão deseja exibir e se as seções **Pastas fixadas** e **Rede** ficam visíveis. Os atalhos padrão dependem das pastas disponíveis no sistema. O destaque considera o caminho completo por componentes e seleciona apenas o ancestral visível mais específico: ao navegar em `Downloads/projeto/media`, por exemplo, **Downloads** continua destacado; se `Downloads/projeto` estiver fixado, ele passa a ser o ancestral mais próximo.

Clique com o botão direito sobre uma pasta na lista e escolha **Fixar na barra lateral**. A seção **Pastas fixadas** também oferece **Fixar pasta atual**, e a aba de configuração aceita um caminho absoluto ou `~/pasta`. Para remover um atalho, use **Desafixar da barra lateral** no menu de contexto ou **Remover** nas configurações. Os atalhos e as escolhas de visibilidade são salvos entre execuções, inclusive para nomes que não são UTF-8. A barra lateral possui rolagem quando as seções excedem a altura disponível.

A seção **Rede** descobre compartilhamentos já montados no Linux, como CIFS/SMB, NFS, SSHFS, WebDAV e rclone, a partir de `/proc/self/mountinfo`; **F5** atualiza a descoberta. Também é possível cadastrar um nome e um ponto de montagem local, como `/mnt/servidor/compartilhamento`, e ocultar montagens automáticas individualmente nas configurações. **Adicionar compartilhamento de rede**, na própria seção, aceita endereços como `smb://servidor/compartilhamento`, `\\servidor\compartilhamento`, `sftp://servidor/pasta`, NFS, FTP e WebDAV, além de pastas locais já montadas. A conexão escolhe o backend disponível do ambiente: **GIO/GVfs no GNOME**, com autenticação GTK nativa em processo auxiliar, ou **KIO Fuse no KDE**. Em conexões SMB, se o KIO falhar ou devolver uma pasta sem acesso à listagem, o explorador tenta GIO/GVfs quando instalado, permitindo autenticação temporária sem exigir que a senha seja salva. Para esse recurso no KDE, instale também `gvfs-backends`, `gvfs-fuse` e GTK 3 ou 4. A listagem da pasta é verificada antes de salvar a conexão ou navegar. As operações rodam em segundo plano, e Cancelar interrompe a espera pela conexão e encerra o auxiliar GNOME. Os módulos de protocolo são fornecidos pelo sistema. Endereço e nome ficam salvos, e clicar na conexão tenta montá-la novamente antes de navegar. Senhas em URLs são rejeitadas e não são gravadas nas preferências. O menu da conexão permite reconectar e remover o atalho; as conexões também aparecem em **Configurações → Barra lateral**. Para listar as pastas de um servidor conhecido, informe apenas `smb://servidor/` ou `\\servidor\` e clique em **Listar compartilhamentos**; selecione um nome na lista e use **Conectar e adicionar**. A listagem, por si só, não cria atalhos nem abre arquivos. **Ctrl+L** também aceita endereços de rede e encaminha a conexão ou a listagem para a mesma janela. A descoberta automática de computadores sem informar um servidor ainda não foi implementada.

A seção **Compartilhamentos locais** da barra lateral mostra as pastas publicadas por este computador como compartilhamentos de usuário do Samba, usando o nome publicado e um ícone de compartilhamento. Clique para abrir a pasta local; o clique direito mantém as ações de pasta e compartilhamento. A lista é carregada em segundo plano e acompanha imediatamente a criação, renomeação e remoção pelo gerenciador, inclusive entre abas. Outras janelas e alterações por ferramentas externas são consultadas automaticamente a cada cinco segundos; **F5** antecipa a consulta. Uma falha preserva a última lista conhecida e exibe um aviso, com o detalhe ao passar o mouse. A visibilidade é independente de Rede em **Configurações → Barra lateral → Exibir seção Compartilhamentos locais**.

O pequeno ícone de engrenagem ao lado do título **Compartilhamentos locais**, com a dica **Gerenciar compartilhamentos locais**, abre a lista de compartilhamentos de usuário do Samba. Selecione um para editar seu nome, descrição e permissões por usuário ou grupo (**Somente leitura**, **Leitura e escrita** ou **Sem acesso**). Os botões de cópia fornecem o endereço SMB e o caminho Windows, com escolha entre os endereços locais disponíveis. O endereço copiado sempre corresponde ao nome já salvo. **Parar de compartilhar** exige confirmação e mantém a pasta e seus arquivos; conexões já abertas podem continuar até serem encerradas.

**Compartilhar pela rede**, no menu de uma pasta, abre esse gerenciador com o compartilhamento existente selecionado ou uma nova configuração para a pasta. Novos compartilhamentos concedem leitura e escrita apenas ao usuário atual, com convidados desativados. Compartilhamentos de outros proprietários aparecem para consulta. A edição preserva a configuração existente de convidados; as permissões de arquivos do Linux continuam se aplicando. Nomes duplicados e alterações externas são detectados antes das mudanças. A renomeação cria o novo nome antes de remover o antigo e tenta desfazer a criação se a remoção falhar, reportando falhas de limpeza. Operações rodam em segundo plano e não solicitam senhas pelo terminal.

O servidor Samba e `net` (`samba-common-bin` no Ubuntu/Debian) precisam estar instalados, com usershares habilitados e permissões do grupo apropriadas (normalmente `sambashare`). O gerenciador altera apenas usershares do usuário, sem editar `/etc/samba/smb.conf`, permissões de arquivos, serviços ou firewall. Compartilhamentos administrativos definidos no arquivo do servidor não fazem parte da lista. **Configurar pelo sistema** mantém acesso às propriedades nativas: **Nautilus/nautilus-share no GNOME** ou **kioclient/kdenetwork-filesharing no KDE**; instalação do servidor e cadastro de senhas continuam sob controle do sistema.

No GNOME, conexões remotas exigem `gvfs-backends`, `gvfs-fuse` e GTK 3 ou GTK 4. No KDE, exigem `kio-fuse` e os módulos adequados de `kio-extras`. Uma montagem existente também pode ser cadastrada. Os arquivos são acessados pelo caminho local fornecido pelo serviço; o suporte em Flatpak/Snap ainda precisa de validação real de permissões e comunicação com o desktop.

Em **Detalhes**, arraste a borda direita de um cabeçalho para ajustar a largura. Arraste o corpo do cabeçalho para mover a coluna; o indicador mostra onde ela será inserida. Um clique simples continua ordenando. A largura e a ordem são salvas automaticamente ao terminar o gesto, inclusive quando uma coluna é ocultada e exibida novamente. Preferências anteriores continuam compatíveis. O modo **Lista** apresenta apenas ícones pequenos e nomes em linhas compactas, sem os cabeçalhos de metadados.

Cada pasta mantém seu modo de visualização, o nível de zoom, as colunas visíveis e sua ordem, as larguras e a ordenação. Essas opções são restauradas ao navegar novamente para a pasta, usar **Voltar/Avançar** ou reiniciar o aplicativo. Atualizar a pasta preserva os ajustes atuais. As opções de colunas e visualização na janela de configurações se aplicam à pasta aberta; tema, ícones e exibição de arquivos ocultos permanecem gerais. Pastas ainda não personalizadas usam a configuração geral anterior como padrão. Os ajustes são associados ao caminho absoluto aberto; caminhos com nomes não UTF-8 também são preservados sem colisões.

Em **Configurações → Colunas**, o botão **Restaurar padrão desta pasta** remove e salva imediatamente apenas a personalização da pasta aberta, reaplicando o padrão geral de visualização, zoom e colunas. As demais pastas, o tema e os ícones permanecem preservados. A restauração também é mantida após reiniciar o aplicativo.

As preferências ficam em `$XDG_CONFIG_HOME/linux-system-explorer/preferences.json`, ou em `~/.config/linux-system-explorer/preferences.json` quando `XDG_CONFIG_HOME` não está definido. A gravação substitui o arquivo atomicamente. Um arquivo inválido é reportado na interface e permanece intacto durante a inicialização.

As colunas incluem nome, tipo, extensão, tamanho, modificação, criação, último acesso, alteração de metadados, proprietário, grupo, UID, GID, permissões, modo octal, inode, links físicos, espaço alocado, blocos de 512 bytes, dispositivo, ausência de bits de escrita, oculto, destino do link, caminho completo, caminho original e data de exclusão. As duas últimas mostram os metadados da lixeira; fora dela aparecem como —. Os campos numéricos são ordenados como números.

- **Criação** usa o nascimento do arquivo, quando disponível; **alteração de metadados** é `ctime`, que não representa a criação. Valores indisponíveis aparecem como `—`.
- **Último acesso** depende das opções de montagem, como `relatime` e `noatime`.
- **Sem bits de escrita** indica apenas os bits POSIX. Não é uma avaliação completa do acesso efetivo por ACLs, privilégios ou montagem.
- **Espaço alocado** usa os blocos reportados pelo sistema multiplicados por 512; pode diferir do tamanho lógico em arquivos esparsos.
- Os metadados de um **link simbólico** descrevem o próprio link, como em `lstat`. A navegação em links para pastas segue o destino. Proprietários e grupos desconhecidos aparecem pelo identificador numérico.

## Verificar

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Os testes cobrem histórico, metadados Linux, permissões, links simbólicos, layouts por pasta, seleção múltipla, atalhos e menus de transferência, cópia e recorte de arquivos reais, conflitos, cancelamento, mudança da origem durante a cópia e movimentação entre sistemas de arquivos. Os testes de abertura exercitam duplo clique e menu de contexto com um abridor isolado, incluindo falhas e nomes especiais. Os testes de ícones cobrem as famílias de arquivos e as três visualizações. Os testes de exclusão e restauração usam arquivos temporários e uma lixeira isolada, cobrindo os atalhos, a confirmação, restauração e exclusão permanente de seleções na Lixeira pelo menu, conflitos, falhas parciais, cancelamento, nomes não UTF-8, origem alterada, movimentação entre sistemas de arquivos e preservação dos destinos de links. Também cobrem o menu Esvaziar lixeira, cancelamento da confirmação, exclusão apenas da lista confirmada, preservação de entradas posteriores e da identificação de itens alterados. Também verificam as traduções e os parâmetros das mensagens, a troca de idioma pela interface e sua persistência, a altura das abas, os botões de zoom nos três modos e o registro desktop do ícone. Também verificam o ancestral mais próximo na barra lateral, visibilidade e pastas fixadas persistentes, menus com ícones personalizados, detecção de montagens de rede, nomes não UTF-8, alinhamento vertical e compatibilidade das preferências.
