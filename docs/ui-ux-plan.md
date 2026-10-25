# Plano de reorganização da interface

Data: 24 de outubro de 2026.
Estado: layout do primeiro marco integrado ao frontend com o tema Glass. O RPM
0.1.0-3 foi preparado; a instalação e a validação na sessão nativa ficam pendentes.

## Direção escolhida

O fluxo principal começa pelo dispositivo: escolher mouse ou teclado, escolher
o perfil de edição e configurar suas ações. Perfis continuam compartilhados entre
dispositivos conforme o modelo atual; selecionar um dispositivo apenas delimita
o que está sendo editado.

## Problemas identificados no código atual

- Início reúne configuração de ações, execução do serviço, ativação de mouse,
  ativação de teclado, integração KWin, estado do daemon e histórico de perfis.
- Dispositivos aparecem no Início e na aba Dispositivos com ações diferentes,
  exigindo que o usuário descubra onde configurar cada aspecto.
- A criação e a associação de perfis ficam separadas da configuração de ações.
- Avisos e indicadores repetidos competem com as ações principais. Um estado
  agregado de entrada ativa não esclarece qual dispositivo está remapeando.
- O mouse abre um editor em modal e outro modal para cada ação; o teclado usa
  outra organização para uma tarefa equivalente.
- Termos técnicos e mensagens antigas aparecem no fluxo comum. O formulário de
  perfis ainda informa incorretamente que nenhuma entrada é remapeada.

Este diagnóstico vem da leitura dos componentes. A avaliação de espaçamento,
legibilidade e aparência deve ser feita também no aplicativo instalado.

## Estrutura proposta

### Dispositivos — tela inicial

- Lista de dispositivos cadastrados, nome, tipo, conexão, bateria quando
  disponível e estado de remapeamento por dispositivo.
- Ação principal de cada item: Configurar. Ativar/Pausar fica acessível no
  mesmo contexto, sem exigir seleção duplicada em outro painel.
- Adicionar dispositivo abre um fluxo guiado de identificação, nome/layout e
  verificação de acesso. Autorizações administrativas continuam explícitas.
- Estados distintos: conectado e pausado, ativo, desconectado, acesso necessário,
  verificando e falha. Não confundir conexão com autorização ou captura ativa.
- Mostrar uma ação de recuperação conforme o impedimento; detalhes de hardware
  e interfaces ficam no cadastro avançado.

### Configuração do dispositivo — página dedicada

- Cabeçalho com nome do dispositivo, perfil em edição e estado real da captura.
- Seletor de perfil e atalho para criar/duplicar perfil sem abandonar o contexto.
- Área central com desenho/lista de botões ou layout do teclado.
- Painel lateral para editar a ação selecionada, com o mesmo padrão de interação
  para mouse e teclado. Evitar empilhar modais para editar uma ação.
- Rodapé de edição com Salvar alterações, Descartar e indicação de rascunho.
- Antes de mudar dispositivo/perfil ou sair, resolver alterações não salvas.
- Diferenciar comportamento original, ação geral e personalização do dispositivo;
  manter a precedência atual e explicar de onde vem a ação efetiva.
- Ações gerais existentes continuam acessíveis em uma opção explícita de escopo,
  sem criar um dispositivo fictício ou perder mapeamentos já salvos.
- Salvar não ativa a captura. O perfil em edição não muda quando outra janela
  recebe foco. Perfil em uso é mostrado como informação separada.

### Perfis

- Lista com nome, aplicativos associados e indicação do perfil Padrão.
- Criar, duplicar, renomear e excluir com comportamento consistente.
- Detectar aplicativo pela janela como caminho principal de associação;
  edição manual de IDs e prioridade ficam nas opções avançadas.
- Duplicar copia as ações de todos os dispositivos do perfil e deixa isso claro.
- O editor do dispositivo pode ser aberto a partir de um perfil já selecionado.
- Histórico e simulador de seleção ficam em diagnóstico, fora da lista principal.

### Configurações e diagnóstico

- Configurações: início do serviço ao entrar no KDE, integração KDE e versão.
- Diagnóstico: estado e controle do serviço, conexão, foco, histórico e simulador.
- Reiniciar serviço é ação de manutenção, com aviso de que a captura será pausada.
- O cabeçalho global mostra um estado compacto. Falhas relevantes exibem a
  explicação e um atalho para resolver, mesmo fora de Configurações.
- Não oferecer ativação automática, novas permissões ou recursos ainda ausentes
  como consequência desta reorganização.

## Regras de interação e linguagem

- Usar Ativar/Pausar remapeamento para captura; Habilitar perfil para participação
  na seleção automática; Iniciar/Parar serviço para o processo em segundo plano.
- Mostrar o estado individual de mouse e teclado; evitar uma indicação global
  que faça um teclado ativo parecer um mouse ativo.
- Preferir mensagens curtas com próximo passo. Exibir erros técnicos em detalhes
  copiáveis quando necessário, preservando informação suficiente para diagnóstico.
- Cada tela tem uma ação principal evidente. Textos de ajuda ficam próximos da
  decisão correspondente, em vez de avisos longos repetidos.
- Remoção de dispositivo, exclusão de perfil e descarte de rascunho exigem
  confirmação proporcional ao que será perdido.
- Estados de carregamento não devem ser apresentados como serviço parado,
  dispositivo desconectado ou autorização negada.

## Sequência de execução

1. Fechar o mapa de telas, terminologia e transições de estado com base nesta
   proposta. Registrar onde cada funcionalidade atual continuará acessível.
2. Produzir protótipos navegáveis de Dispositivos, editor de mouse, editor de
   teclado e Perfis. Incluir falta de acesso, serviço parado e rascunho pendente.
   Validar a hierarquia visual antes da implementação completa.
3. Implementar navegação e estado compartilhado, reaproveitando APIs existentes.
   Consolidar leituras de estado para evitar consultas e assinaturas duplicadas.
4. Migrar os editores para página e painel lateral, com tratamento consistente
   de rascunhos e preservação de revisão/conflitos ao salvar.
5. Reorganizar perfis, cadastro/acesso e diagnóstico. Remover textos obsoletos,
   duplicações e caminhos antigos após confirmar cobertura funcional.
6. Validar no navegador e no aplicativo nativo; gerar RPM com revisão nova e
   verificar os fluxos reais após instalação.

## Critérios de conclusão

- Da tela inicial, abrir o editor de um dispositivo exige um clique.
- O usuário sabe qual dispositivo e perfil está editando e se eles estão em uso.
- Ativar/Pausar e resolver uma falta de acesso são acessíveis no contexto do
  dispositivo, sem navegar por painéis técnicos.
- Editar uma ação não exige uma sequência de modais sobrepostos.
- Trocar de tela/perfil não perde rascunhos silenciosamente nem deixa detecção
  de janela armada em uma tela oculta.
- Configurações e ações existentes são preservadas, incluindo perfis com regras
  avançadas, ações gerais e precedência específica por interface.
- Navegação por teclado, foco visível, rótulos e estados anunciados funcionam;
  estado não depende apenas de cor. Validar na janela padrão de 960 × 800 e com
  escala de texto ampliada, acomodando o teclado sem cortar ações essenciais.
- Parada/reinício do serviço, bloqueio, desconexão e revogação de acesso continuam
  refletidos corretamente na UI e na bandeja, sem reativação automática.
- Testes relevantes de comportamento, build web e nativo e inspeção visual passam.
  A reorganização da UI não adiciona trabalho ao leitor de entrada.

## Primeiro marco

Revisar um protótipo navegável do fluxo Dispositivos → Configurar → Salvar,
incluindo o perfil em edição e os estados ativo, pausado e sem acesso.
Após validar esse fluxo, estender o mesmo padrão ao restante do aplicativo.

### Prévia produzida

Foi criada uma prévia interativa com dados demonstrativos do Logitech G PRO
Wireless e Aula F99, perfis Padrão, Arc Raiders e Trabalho. Inclui navegação entre
Dispositivos, Perfis e Configurações, editores de mouse e teclado com painel
lateral, salvamento/descarte, proteção de rascunhos e simulação de autorização.

Foram conferidos no navegador o salvamento mantendo o estado de captura, o
bloqueio de navegação com alterações pendentes, a autorização mantendo o teclado
pausado e o reinício deixando ambos os dispositivos pausados. O layout foi
inspecionado em larguras de 960px e 360px. São verificações do protótipo; não
validam as APIs nativas nem o comportamento do RPM.

A prévia não acessa dispositivos, serviço, permissões ou configurações reais.
O teclado representado é simplificado para avaliar a interação; a implementação
deve reutilizar os layouts completos já existentes. Associação de aplicativos,
ações gerais e diagnóstico completo continuam previstos para as próximas etapas.
O próximo passo é revisar a hierarquia e o fluxo desta prévia antes de migrar
os componentes de produção.

### Integração do layout e tema Glass

Após a escolha das referências visuais e a preferência explícita pelo layout
da prévia, o frontend passou a usar três áreas: Dispositivos, Perfis e
Configurações. Cadastro e interfaces continuam acessíveis pelas opções de
Dispositivos; adicionar abre o formulário diretamente. Os cartões reúnem
Configurar, Ativar/Pausar e Acesso, com estado individual.

Os editores de mouse e teclado agora abrem em página com painel lateral. O
perfil em edição fica separado do perfil em uso. Salvar mantém a edição aberta;
voltar, mudar de perfil ou área exige resolver o rascunho pendente. Atalhos
incompletos são preservados no rascunho e impedem salvar. As regras de revisão,
escopo por interface e ações gerais existentes são reaproveitadas.

A revisão no navegador usou componentes reais com APIs em memória. Foram
validados salvamento de mouse e teclado sem ativação, proteção do rascunho,
troca de perfil e retorno do foco. A janela padrão de 960 × 800 e uma largura
de 390px foram inspecionadas. Os 30 testes do frontend e o build nativo/RPM
passaram. A autorização e a captura física não foram acionadas nesta rodada.
