# 4. Deploying Your Agent

An agent running on your local machine is great for development, but a real agent needs to run 24/7 on a server. This tutorial will guide you through packaging your "Echo Agent" from the previous tutorial into a Docker container and deploying it.

## Prerequisites

-   You have completed the "Building a Complete Agent" tutorial.
-   You have [Docker](https://www.docker.com/get-started) installed on your machine.
-   You have an account with a cloud provider (e.g., [AWS](https://aws.amazon.com/), [Google Cloud](https://cloud.google.com/), [Azure](https://azure.microsoft.com/)) or a container hosting service (e.g., [Railway](https://railway.app/), [Render](https://render.com/)). This tutorial will provide general guidance.

## Step 1: Containerize the Agent with Docker

A Docker container packages your application and all its dependencies into a standardized unit for software development.

1.  **Create a `Dockerfile`**: In the root of your `echo-agent` project, create a file named `Dockerfile` (no extension).

2.  **Add the following content to your `Dockerfile`**:

    ```Dockerfile
    # Use an official Node.js runtime as a parent image
    FROM node:18-slim

    # Set the working directory in the container
    WORKDIR /usr/src/app

    # Copy package.json and package-lock.json
    COPY package*.json ./

    # Install app dependencies
    RUN npm install

    # Bundle app source
    COPY . .

    # Your app binds to port 8080 so you can use it with any PaaS
    # (This is not strictly necessary for our agent, but it's good practice)
    EXPOSE 8080

    # Define the command to run your app
    CMD [ "node", "index.js" ]
    ```

3.  **Create a `.dockerignore` file**: This file prevents certain files from being copied into your container, keeping it lightweight. Create a `.dockerignore` file with the following content:

    ```
    node_modules
    .env
    Dockerfile
    .dockerignore
    ```

## Step 2: Build and Test the Docker Image

1.  **Build the image**: From your terminal, in the root of your project, run the `docker build` command:

    ```bash
    docker build -t echo-agent .
    ```
    This command builds a Docker image named `echo-agent` based on your `Dockerfile`.

2.  **Run the container**: Now, run the image you just built. You need to pass in your private key as an environment variable.

    ```bash
    # Replace sk1... with your actual private key
    docker run -e NOSTR_PRIVATE_KEY=sk1... echo-agent
    ```

3.  **Test it**: The agent should now be running inside the container. You can test it the same way you did in the previous tutorial: send it an "echo" message from a Nostr client and wait for the "echo back" reply.

## Step 3: Deploy to the Cloud

Now that you have a working Docker image, you can deploy it to any service that supports running containers. The general steps are similar across most platforms.

### General Deployment Steps:

1.  **Push your image to a registry**:
    *   You'll need to push your Docker image to a container registry like [Docker Hub](https://hub.docker.com/), [Amazon ECR](https://aws.amazon.com/ecr/), or [Google Container Registry](https://cloud.google.com/container-registry).
    *   This usually involves tagging your image with the registry's address and then running `docker push`.
    *   Example for Docker Hub:
        ```bash
        docker tag echo-agent your-dockerhub-username/echo-agent
        docker push your-dockerhub-username/echo-agent
        ```

2.  **Configure your cloud service**:
    *   In your chosen cloud provider's console, create a new service or application.
    *   Point the service to the Docker image you just pushed to the registry.
    *   **Crucially, you must configure the environment variables for your service.** Find the section for "Environment Variables" or "Secrets" and add a variable named `NOSTR_PRIVATE_KEY` with your agent's private key as the value. **Always use the "secret" or "encrypted" option if available.**

3.  **Launch the service**:
    *   Deploy the service. The cloud provider will pull your image and run it as a container.
    *   Check the logs in your provider's dashboard to ensure the agent started correctly.

### Example: Deploying with Railway

[Railway](https://railway.app/) is a simple platform for deploying containerized applications.
1.  Install the Railway CLI.
2.  Run `railway login`.
3.  Run `railway init` in your project directory.
4.  Run `railway up`. Railway will automatically detect your `Dockerfile`, build the image, and deploy it.
5.  Go to your project's dashboard on Railway, navigate to "Variables", and add your `NOSTR_PRIVATE_KEY` as a secret. The service will automatically restart with the new variable.

## Congratulations!

You have now deployed a persistent, 24/7 agent to the cloud. Your agent is now a permanent resident of the decentralized web, ready to provide services to other agents.

This is the final step in the basic agent development lifecycle. You are now equipped with the knowledge to create, test, and deploy sophisticated agents on the SAND stack.

---
**Previous:** [3. Building a Complete Agent](./../03-complete-agent/README.md)